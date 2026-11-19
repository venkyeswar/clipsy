#!/bin/sh
# Clipsy per-user uninstaller. Removes a Clipsy installed by install.sh (no sudo).
#
# It deletes exactly the files listed in the install manifest that install.sh
# wrote, and the folders install.sh created if they are now empty. Your clipboard
# history is kept unless you ask for it to be deleted.
#
#   sh ~/.local/share/clipsy/uninstall.sh
#
# Run `sh uninstall.sh --help` for the options.

set -eu

usage() {
    cat <<'EOF'
Remove Clipsy that was installed into your home folder (no sudo needed).

Usage: sh uninstall.sh [options]

Options:
  --prefix DIR    Clipsy was installed under DIR instead of ~/.local
  --keep-data     Keep your clipboard history and settings (the default).
  --delete-data   Also delete your clipboard history, settings and the
                  "start at login" entry.
  --help          Show this help.

Without --keep-data or --delete-data you are asked, when run in a terminal.
EOF
}

say() { printf '%s\n' "$*"; }
die() {
    printf '\nError: %s\n' "$*" >&2
    exit 1
}

home="${HOME:?HOME is not set}"
prefix="$home/.local"
data_mode="" # "keep", "delete", or empty (= ask)

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            [ $# -ge 2 ] || die "--prefix needs a folder."
            prefix="$2"
            shift 2
            ;;
        --keep-data)
            data_mode="keep"
            shift
            ;;
        --delete-data)
            data_mode="delete"
            shift
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

# The prefix must be a real absolute folder: everything we delete is inside it.
prefix="${prefix%/}"
case "$prefix" in
    /?*) ;;
    *) die "--prefix must be an absolute folder, for example $home/.local" ;;
esac
case "$home" in
    /?*) ;;
    *) die "HOME must be an absolute folder." ;;
esac

state_dir="$prefix/share/clipsy"
manifest="$state_dir/install-manifest"
created_dirs="$state_dir/created-dirs"
[ -f "$manifest" ] || die "no Clipsy install was found in $prefix (there is no $manifest).
If you installed Clipsy another way: Snap: sudo snap remove clipsy   .deb: sudo apt remove clipsy"

# Where Clipsy keeps your data (the same rules the app uses).
data_dir="${XDG_DATA_HOME:-$home/.local/share}/clipsy"
config_dir="${XDG_CONFIG_HOME:-$home/.config}/clipsy"
autostart="${XDG_CONFIG_HOME:-$home/.config}/autostart/io.github.clipsy.Clipsy.desktop"

# ---------------------------------------------------------------------------
# Stop Clipsy if this copy of it is running
# ---------------------------------------------------------------------------
# Only ask our own binary to quit (checked through /proc), so a Snap or .deb
# Clipsy that happens to be running is left alone.
binary="$prefix/bin/clipsy"
if [ -x "$binary" ] && command -v pgrep >/dev/null 2>&1; then
    for pid in $(pgrep -u "$(id -u)" -x clipsy 2>/dev/null || true); do
        exe="$(readlink "/proc/$pid/exe" 2>/dev/null || true)"
        case "$exe" in
            "$binary" | "$binary (deleted)")
                say "Stopping Clipsy..."
                "$binary" quit >/dev/null 2>&1 || true
                break
                ;;
        esac
    done
fi

# ---------------------------------------------------------------------------
# Ask about data
# ---------------------------------------------------------------------------
if [ -z "$data_mode" ]; then
    data_mode="keep"
    if [ -t 0 ]; then
        say "Do you also want to delete your clipboard history and settings?"
        say "  $data_dir  (history and images)"
        say "  $config_dir  (settings)"
        printf 'Delete them? [y/N] '
        answer=""
        read -r answer || true
        case "$answer" in
            y | Y | yes | YES | Yes) data_mode="delete" ;;
        esac
    fi
fi

# ---------------------------------------------------------------------------
# Remove the installed files
# ---------------------------------------------------------------------------
say "Removing Clipsy from $prefix..."
while IFS= read -r path; do
    case "$path" in
        "$prefix"/?*) ;;
        *) continue ;; # never delete anything outside the prefix
    esac
    if [ -f "$path" ] || [ -L "$path" ]; then
        rm -f "$path"
    fi
done <"$manifest"
rm -f "$manifest"

# Refresh the menu so Clipsy disappears from it. Failures do not matter.
if [ -d "$prefix/share/applications" ] && command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q "$prefix/share/applications" >/dev/null 2>&1 || true
fi

# Remove folders install.sh created, deepest first, but only when empty.
if [ -f "$created_dirs" ]; then
    dirs="$(awk '{ line[NR] = $0 } END { for (i = NR; i > 0; i--) print line[i] }' "$created_dirs")"
    rm -f "$created_dirs"
    printf '%s\n' "$dirs" | while IFS= read -r dir; do
        case "$dir" in
            "$prefix" | "$prefix"/?*) ;;
            *) continue ;;
        esac
        [ -d "$dir" ] || continue
        # The menu cache file is all that is left in an applications folder we made.
        if [ "$dir" = "$prefix/share/applications" ] &&
            [ "$(ls -A "$dir")" = "mimeinfo.cache" ]; then
            rm -f "$dir/mimeinfo.cache"
        fi
        rmdir "$dir" 2>/dev/null || true
    done
fi

# ---------------------------------------------------------------------------
# Delete data if asked
# ---------------------------------------------------------------------------
# remove_tree DIR: rm -rf, but only for a non-empty path that ends in /clipsy.
remove_tree() {
    case "$1" in
        /?*/clipsy)
            if [ -d "$1" ]; then
                rm -rf -- "$1"
                say "Deleted $1"
            fi
            ;;
        *) say "Skipped $1 (unexpected path)" ;;
    esac
}

if [ "$data_mode" = "delete" ]; then
    remove_tree "$data_dir"
    remove_tree "$config_dir"
    if [ -f "$autostart" ]; then
        rm -f "$autostart"
        say "Deleted $autostart"
    fi
    say ""
    say "Done. Clipsy and your clipboard history are removed."
else
    say ""
    say "Done. Clipsy is removed."
    if [ -d "$data_dir" ] || [ -d "$config_dir" ]; then
        say "Your clipboard history is kept in $data_dir and your settings in $config_dir."
        say "To delete them too, run: rm -r $data_dir $config_dir"
    fi
    if [ -f "$autostart" ]; then
        say "Clipsy's \"start at login\" entry is still in $autostart. To remove it: rm $autostart"
    fi
fi
