#!/bin/sh
# Clipsy per-user installer. Needs no sudo.
#
# It downloads the Clipsy .deb from GitHub, checks its SHA256 checksum, unpacks it
# with `dpkg-deb -x` (which needs no root) and copies the app into your home folder
# (by default ~/.local). Running it again updates Clipsy in place.
#
#   curl -fsSL https://github.com/venkyeswar/clipsy/releases/latest/download/install.sh -o install.sh
#   sh install.sh
#
# Run `sh install.sh --help` for the options.

set -eu

REPO="venkyeswar/clipsy"
# Base URL of the releases. Can be overridden (for tests or mirrors).
RELEASES_BASE="${CLIPSY_RELEASES_BASE:-https://github.com/$REPO/releases}"
APP_ID="io.github.clipsy.Clipsy"

usage() {
    cat <<'EOF'
Install Clipsy into your home folder (no sudo needed).

Usage: sh install.sh [options]

Options:
  --deb FILE        Install from a .deb file you already downloaded (no network).
  --version X.Y.Z   Install that release instead of the latest one.
  --prefix DIR      Install under DIR instead of ~/.local
  --help            Show this help.

Run the same command again later to update Clipsy.
To remove it: sh ~/.local/share/clipsy/uninstall.sh
EOF
}

say() { printf '%s\n' "$*"; }
die() {
    printf '\nError: %s\n' "$*" >&2
    exit 1
}

# ---------------------------------------------------------------------------
# Options
# ---------------------------------------------------------------------------
deb_file=""
version=""
prefix="${HOME:?HOME is not set}/.local"

while [ $# -gt 0 ]; do
    case "$1" in
        --deb)
            [ $# -ge 2 ] || die "--deb needs a file name."
            deb_file="$2"
            shift 2
            ;;
        --version)
            [ $# -ge 2 ] || die "--version needs a version number, for example 0.3.2"
            version="${2#v}"
            shift 2
            ;;
        --prefix)
            [ $# -ge 2 ] || die "--prefix needs a folder."
            prefix="$2"
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            usage >&2
            die "unknown option: $1"
            ;;
    esac
done

# Remove a trailing slash so paths look tidy, then make sure the prefix is a real,
# absolute folder name. Everything this script writes lives under it.
prefix="${prefix%/}"
case "$prefix" in
    /?*) ;;
    *) die "--prefix must be an absolute folder, for example $HOME/.local" ;;
esac
if [ -n "$deb_file" ] && [ ! -f "$deb_file" ]; then
    die "the file '$deb_file' does not exist."
fi

# ---------------------------------------------------------------------------
# Checks before doing anything
# ---------------------------------------------------------------------------
if [ "$(id -u)" -eq 0 ]; then
    say "Note: you are running this as root, so Clipsy is installed for root only ($prefix)."
    say "      For a normal install, run it as your own user, without sudo."
fi

arch="$(uname -m)"
case "$arch" in
    x86_64 | amd64) ;;
    *) die "Clipsy is only built for 64-bit Intel/AMD computers (x86_64), and this one is '$arch'." ;;
esac

# curl or wget is needed to download (with --deb it is only used to fetch the
# small uninstaller, and is optional).
downloader=""
if command -v curl >/dev/null 2>&1; then
    downloader="curl"
elif command -v wget >/dev/null 2>&1; then
    downloader="wget"
fi
if [ -z "$deb_file" ]; then
    [ -n "$downloader" ] || die "this script needs curl or wget to download Clipsy. Ask an admin to run: sudo apt install curl"
    command -v sha256sum >/dev/null 2>&1 || die "sha256sum is missing (it is part of coreutils)."
fi
command -v dpkg-deb >/dev/null 2>&1 || die "dpkg-deb is missing. This installer works on Debian and Ubuntu based systems."

# Clipsy uses GTK4 and libadwaita. They come with Ubuntu 24.04 GNOME, but
# installing them needs an admin, so stop here and say exactly what to ask for.
missing_libs=""
libs="$(ldconfig -p 2>/dev/null || /sbin/ldconfig -p 2>/dev/null || true)"
printf '%s\n' "$libs" | grep -q 'libgtk-4\.so' || missing_libs="GTK4"
printf '%s\n' "$libs" | grep -q 'libadwaita-1\.so' || missing_libs="${missing_libs:+$missing_libs and }libadwaita"
if [ -n "$missing_libs" ]; then
    die "$missing_libs is not installed on this computer, and Clipsy needs it.
Installing it needs an admin, but only once. Ask them to run:

    sudo apt install libgtk-4-1 libadwaita-1-0

Then run this installer again."
fi

# ---------------------------------------------------------------------------
# Download helpers
# ---------------------------------------------------------------------------
# download URL FILE: save URL to FILE, failing on HTTP errors.
download() {
    if [ "$downloader" = "curl" ]; then
        curl -fsSL --retry 2 -o "$2" "$1"
    else
        wget -q -O "$2" "$1"
    fi
}

# Prints the latest release version (like 0.3.2). GitHub redirects
# .../releases/latest to .../releases/tag/vX.Y.Z, so we follow that redirect
# instead of using the GitHub API (which has a low rate limit).
latest_version() {
    if [ "$downloader" = "curl" ]; then
        url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$RELEASES_BASE/latest")" || return 1
    else
        url="$(wget -q -S --spider --max-redirect=5 "$RELEASES_BASE/latest" 2>&1 |
            sed -n 's/^ *[Ll]ocation: *//p' | tail -n 1 | tr -d '\r')" || return 1
    fi
    tag="${url##*/}"
    case "$tag" in
        v[0-9]*) printf '%s\n' "${tag#v}" ;;
        *) return 1 ;;
    esac
}

# ---------------------------------------------------------------------------
# Get and unpack the package
# ---------------------------------------------------------------------------
work="$(mktemp -d "${TMPDIR:-/tmp}/clipsy-install.XXXXXX")"
# Remove the temporary folder however the script ends.
trap 'rm -rf "$work"' EXIT
trap 'exit 1' INT TERM HUP

if [ -n "$deb_file" ]; then
    say "Installing from $deb_file"
    deb="$deb_file"
else
    if [ -z "$version" ]; then
        say "Finding the latest Clipsy version..."
        version="$(latest_version)" || die "could not find the latest version. Check your internet connection, or pick one with --version X.Y.Z"
    fi
    deb_name="clipsy_${version}-1_amd64.deb"
    release_url="$RELEASES_BASE/download/v$version"
    say "Downloading Clipsy $version..."
    download "$release_url/$deb_name" "$work/$deb_name" ||
        die "could not download $release_url/$deb_name"
    download "$release_url/SHA256SUMS" "$work/SHA256SUMS" ||
        die "could not download $release_url/SHA256SUMS"

    say "Checking the download..."
    # Keep only the line for our file, then let sha256sum compare it.
    grep " \*\{0,1\}$deb_name\$" "$work/SHA256SUMS" >"$work/expected" ||
        die "SHA256SUMS has no entry for $deb_name, so the download cannot be checked."
    if ! (cd "$work" && sha256sum -c --quiet expected) >/dev/null 2>&1; then
        die "the downloaded file does not match its checksum. It may be damaged or tampered with.
Nothing was installed. Try again later."
    fi
    deb="$work/$deb_name"
fi

dpkg-deb -x "$deb" "$work/pkg" || die "could not unpack $deb. Is it a Clipsy .deb file?"
pkg="$work/pkg/usr"
[ -x "$pkg/bin/clipsy" ] || die "$deb does not contain usr/bin/clipsy. Is it a Clipsy .deb file?"
installed_version="$("$pkg/bin/clipsy" --version 2>/dev/null | sed -n 's/^clipsy //p')"
[ -n "$installed_version" ] || installed_version="${version:-unknown}"

# ---------------------------------------------------------------------------
# Copy into the prefix
# ---------------------------------------------------------------------------
state_dir="$prefix/share/clipsy"
manifest="$state_dir/install-manifest"
created_dirs="$state_dir/created-dirs"
new_manifest="$work/manifest"
new_dirs="$work/created-dirs"
: >"$new_manifest"
: >"$new_dirs"

# make_dir DIR: like `mkdir -p`, but remembers each folder it had to create so
# the uninstaller removes only those (and only if they end up empty).
make_dir() {
    if [ ! -d "$1" ]; then
        make_dir "$(dirname "$1")"
        mkdir "$1"
        printf '%s\n' "$1" >>"$new_dirs"
    fi
}

# put SOURCE DEST MODE: copy next to DEST first and then rename over it, so a
# running Clipsy is never left with a half-written file.
put() {
    make_dir "$(dirname "$2")"
    cp "$1" "$2.new"
    chmod "$3" "$2.new"
    mv -f "$2.new" "$2"
    printf '%s\n' "$2" >>"$new_manifest"
}

say "Installing into $prefix..."

put "$pkg/bin/clipsy" "$prefix/bin/clipsy" 0755

# Menu entry. The packaged one runs `clipsy` from PATH, which may not include
# ~/.local/bin yet, so point every Exec= line at the full path (keeping any
# arguments, like `clipsy settings`).
# Escape the characters that are special on the right side of a sed s|||.
exec_path="$(printf '%s' "$prefix/bin/clipsy" | sed 's/[\\|&]/\\&/g')"
for desktop in "$pkg"/share/applications/*.desktop; do
    [ -f "$desktop" ] || continue
    name="$(basename "$desktop")"
    sed -e "s|^Exec=clipsy\$|Exec=\"$exec_path\"|" \
        -e "s|^Exec=clipsy |Exec=\"$exec_path\" |" "$desktop" >"$work/$name"
    put "$work/$name" "$prefix/share/applications/$name" 0644
done

# Icons, keeping their folder layout (hicolor/scalable/apps/...).
if [ -d "$pkg/share/icons" ]; then
    (cd "$pkg/share/icons" && find . -type f) | while IFS= read -r icon; do
        icon="${icon#./}"
        put "$pkg/share/icons/$icon" "$prefix/share/icons/$icon" 0644
    done
fi

for meta in "$pkg"/share/metainfo/*.xml; do
    [ -f "$meta" ] || continue
    put "$meta" "$prefix/share/metainfo/$(basename "$meta")" 0644
done

# The uninstaller lives next to the manifest. Use the copy next to this script
# when there is one (and it really is Clipsy's), otherwise download it from the
# same release, falling back to the latest one.
here="$(cd "$(dirname "$0")" 2>/dev/null && pwd || true)"
if [ -n "$here" ] && [ -f "$here/uninstall.sh" ] && grep -q 'Clipsy per-user uninstaller' "$here/uninstall.sh"; then
    cp "$here/uninstall.sh" "$work/uninstall.sh"
elif [ -n "$downloader" ]; then
    download "$RELEASES_BASE/download/v$installed_version/uninstall.sh" "$work/uninstall.sh" 2>/dev/null ||
        download "$RELEASES_BASE/latest/download/uninstall.sh" "$work/uninstall.sh" 2>/dev/null ||
        rm -f "$work/uninstall.sh"
fi
uninstaller_ok=0
if [ -f "$work/uninstall.sh" ] && grep -q 'Clipsy per-user uninstaller' "$work/uninstall.sh"; then
    put "$work/uninstall.sh" "$state_dir/uninstall.sh" 0755
    uninstaller_ok=1
fi

printf '%s\n' "$installed_version" >"$work/version"
put "$work/version" "$state_dir/version" 0644

# Upgrading: delete files the old version installed that this one no longer has.
if [ -f "$manifest" ]; then
    while IFS= read -r old; do
        case "$old" in
            "$prefix"/*) ;;
            *) continue ;; # never touch anything outside the prefix
        esac
        if ! grep -qxF "$old" "$new_manifest" && [ -f "$old" ]; then
            rm -f "$old"
        fi
    done <"$manifest"
fi
# Keep folders recorded by an earlier install, plus any created just now.
[ -f "$created_dirs" ] && cat "$created_dirs" >>"$new_dirs"
awk '!seen[$0]++' "$new_dirs" >"$work/dirs-unique"
cp "$work/dirs-unique" "$created_dirs"
cp "$new_manifest" "$manifest"

# Refresh the menu caches when the tools are there. Failures do not matter.
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q "$prefix/share/applications" >/dev/null 2>&1 || true
fi
# Only refresh the icon cache if the theme folder already has an index (it
# normally does not under ~/.local, and GNOME finds the icon without a cache).
if command -v gtk-update-icon-cache >/dev/null 2>&1 && [ -f "$prefix/share/icons/hicolor/index.theme" ]; then
    gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" >/dev/null 2>&1 || true
fi

# ---------------------------------------------------------------------------
# Tell the user what happened and what to do next
# ---------------------------------------------------------------------------
if [ "$prefix" = "$HOME/.local" ]; then
    uninstall_cmd="sh ~/.local/share/clipsy/uninstall.sh"
else
    uninstall_cmd="sh $state_dir/uninstall.sh --prefix $prefix"
fi

say ""
say "Done. Clipsy $installed_version is installed."
say "  Program:  $prefix/bin/clipsy"
say ""
say "Start it:   find Clipsy in your app menu, or run $prefix/bin/clipsy"
case ":$PATH:" in
    *":$prefix/bin:"*)
        say "            (or just type: clipsy)"
        ;;
    *)
        say ""
        say "$prefix/bin is not in your PATH yet, so typing 'clipsy' will not work."
        if [ "$prefix" = "$HOME/.local" ]; then
            say "On Ubuntu it is added after you log out and back in. If it still is not, run:"
            say "    echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.profile"
        else
            say "To add it, run:"
            say "    echo 'export PATH=\"$prefix/bin:\$PATH\"' >> ~/.profile"
        fi
        say "and log out and back in."
        ;;
esac
say ""
say "Open the clipboard history with Super+Shift+V."
say "If Clipsy was already running, restart it: $prefix/bin/clipsy quit, then start it again."
say ""
say "Update:     run this installer again."
if [ "$uninstaller_ok" -eq 1 ]; then
    say "Uninstall:  $uninstall_cmd"
else
    say "Uninstall:  download $RELEASES_BASE/latest/download/uninstall.sh and run: sh uninstall.sh"
fi
