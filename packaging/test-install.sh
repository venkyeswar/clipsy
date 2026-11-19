#!/bin/sh
# Tests for install.sh and uninstall.sh.
#
#   sh packaging/test-install.sh [path/to/clipsy.deb]
#
# Everything happens inside a temporary folder: HOME and the install prefix point
# there, so your real ~/.local and a running Clipsy are never touched. The only
# program it runs from the package is `clipsy --version`.
#
# Set CLIPSY_TEST_OFFLINE=1 to skip the one test that downloads the real release.

set -eu

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"
deb="${1:-$(ls -t "$repo"/target/debian/clipsy_*_amd64.deb 2>/dev/null | head -n 1)}"
[ -n "$deb" ] && [ -f "$deb" ] || {
    echo "No .deb found. Build one with packaging/build-deb.sh or pass its path." >&2
    exit 1
}
deb_name="$(basename "$deb")"
deb_version="$(printf '%s' "$deb_name" | sed 's/^clipsy_\(.*\)-1_amd64\.deb$/\1/')"

scratch="$(mktemp -d "${TMPDIR:-/tmp}/clipsy-test.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT

# Isolate from the real session: a fake home and no desktop/bus to talk to.
export HOME="$scratch/home"
mkdir -p "$HOME"
unset XDG_DATA_HOME XDG_CONFIG_HOME DBUS_SESSION_BUS_ADDRESS DISPLAY WAYLAND_DISPLAY || true

passed=0
fail() {
    echo "FAIL: $*" >&2
    exit 1
}
ok() {
    passed=$((passed + 1))
    echo "ok   $*"
}

install_sh() { sh "$here/install.sh" "$@" </dev/null >"$scratch/out" 2>&1; }
uninstall_sh() { sh "$here/uninstall.sh" "$@" </dev/null >"$scratch/out" 2>&1; }
show_output() { sed 's/^/     | /' "$scratch/out" >&2; }

# Folder is gone, or exists but has nothing in it.
assert_empty() {
    if [ -d "$1" ] && [ -n "$(find "$1" -mindepth 1 -print -quit)" ]; then
        find "$1" -mindepth 1 >&2
        fail "$1 still has files in it"
    fi
}

# Checks a finished install under prefix $1.
assert_installed() {
    p="$1"
    [ -x "$p/bin/clipsy" ] || fail "$p/bin/clipsy is missing or not executable"
    "$p/bin/clipsy" --version | grep -q "^clipsy $deb_version\$" || fail "clipsy --version is wrong"
    desktop="$p/share/applications/io.github.clipsy.Clipsy.desktop"
    [ -f "$desktop" ] || fail "desktop file missing"
    grep -qxF "Exec=\"$p/bin/clipsy\"" "$desktop" || fail "main Exec= does not point into the prefix"
    grep -qxF "Exec=\"$p/bin/clipsy\" settings" "$desktop" || fail "Settings Exec= lost its argument"
    if grep '^Exec=' "$desktop" | grep -vqF "Exec=\"$p/bin/clipsy\""; then
        fail "an Exec= line does not point into the prefix"
    fi
    [ -f "$p/share/icons/hicolor/scalable/apps/io.github.clipsy.Clipsy.svg" ] || fail "icon missing"
    [ -f "$p/share/metainfo/io.github.clipsy.Clipsy.metainfo.xml" ] || fail "metainfo missing"
    [ "$(cat "$p/share/clipsy/version")" = "$deb_version" ] || fail "version file is wrong"
    manifest="$p/share/clipsy/install-manifest"
    [ -s "$manifest" ] || fail "manifest missing"
    while IFS= read -r f; do
        [ -f "$f" ] || fail "manifest lists $f but it does not exist"
        case "$f" in "$p"/*) ;; *) fail "manifest entry outside prefix: $f" ;; esac
    done <"$manifest"
}

# 1. Install from a local .deb into a fresh prefix.
prefix="$scratch/prefix"
install_sh --deb "$deb" --prefix "$prefix" || { show_output; fail "install from --deb failed"; }
assert_installed "$prefix"
[ -x "$prefix/share/clipsy/uninstall.sh" ] || fail "uninstall.sh was not copied next to the manifest"
grep -q "Done. Clipsy $deb_version is installed." "$scratch/out" || fail "no Done message"
grep -q "not in your PATH" "$scratch/out" || fail "no PATH hint for a prefix outside PATH"
cp "$prefix/share/clipsy/install-manifest" "$scratch/manifest-1"
ok "installs from --deb, binary runs, Exec= points into the prefix"

# 2. Running it again upgrades in place and gives the same result.
install_sh --deb "$deb" --prefix "$prefix" || { show_output; fail "second install failed"; }
assert_installed "$prefix"
cmp -s "$scratch/manifest-1" "$prefix/share/clipsy/install-manifest" || fail "manifest changed on re-run"
[ -z "$(find "$prefix" -name '*.new')" ] || fail "left temporary .new files"
ok "re-running replaces the install in place"

# 3. A file from an "older version" that is no longer shipped is removed on upgrade.
echo old >"$prefix/share/metainfo/old-file.xml"
echo "$prefix/share/metainfo/old-file.xml" >>"$prefix/share/clipsy/install-manifest"
install_sh --deb "$deb" --prefix "$prefix" || { show_output; fail "upgrade install failed"; }
[ ! -e "$prefix/share/metainfo/old-file.xml" ] || fail "stale file from old manifest was kept"
ok "upgrade removes files the old manifest had but the new one does not"

# Fake user data, as the app would create it.
make_data() {
    mkdir -p "$HOME/.local/share/clipsy/images" "$HOME/.config/clipsy" "$HOME/.config/autostart"
    echo db >"$HOME/.local/share/clipsy/history.db"
    echo img >"$HOME/.local/share/clipsy/images/1.png"
    echo cfg >"$HOME/.config/clipsy/config.toml"
    echo entry >"$HOME/.config/autostart/io.github.clipsy.Clipsy.desktop"
}

# 4. Uninstall with --keep-data removes every installed file and keeps the data.
make_data
uninstall_sh --prefix "$prefix" --keep-data || { show_output; fail "uninstall --keep-data failed"; }
while IFS= read -r f; do
    [ ! -e "$f" ] || fail "uninstall left $f"
done <"$scratch/manifest-1"
assert_empty "$prefix"
[ -f "$HOME/.local/share/clipsy/history.db" ] || fail "--keep-data deleted the history"
[ -f "$HOME/.config/clipsy/config.toml" ] || fail "--keep-data deleted the settings"
[ -f "$HOME/.config/autostart/io.github.clipsy.Clipsy.desktop" ] || fail "--keep-data deleted autostart"
ok "uninstall --keep-data removes every manifest file, leaves no files, keeps data"

# 5. Uninstall with no flag and no terminal keeps the data too.
install_sh --deb "$deb" --prefix "$prefix" || { show_output; fail "reinstall failed"; }
uninstall_sh --prefix "$prefix" || { show_output; fail "uninstall (no flag) failed"; }
assert_empty "$prefix"
[ -f "$HOME/.local/share/clipsy/history.db" ] || fail "uninstall without a terminal deleted data"
ok "uninstall without a flag or terminal keeps data"

# 6. Uninstall with --delete-data removes data, settings and autostart.
install_sh --deb "$deb" --prefix "$prefix" || { show_output; fail "reinstall failed"; }
uninstall_sh --prefix "$prefix" --delete-data || { show_output; fail "uninstall --delete-data failed"; }
assert_empty "$prefix"
[ ! -e "$HOME/.local/share/clipsy" ] || fail "--delete-data kept the data folder"
[ ! -e "$HOME/.config/clipsy" ] || fail "--delete-data kept the settings folder"
[ ! -e "$HOME/.config/autostart/io.github.clipsy.Clipsy.desktop" ] || fail "--delete-data kept autostart"
ok "uninstall --delete-data removes history, settings and autostart"

# 7. Default prefix (~/.local of the fake HOME), where the install files share the
#    data folder: keeping data must keep the history but drop the install files.
make_data
install_sh --deb "$deb" || { show_output; fail "install with default prefix failed"; }
assert_installed "$HOME/.local"
sh "$HOME/.local/share/clipsy/uninstall.sh" --keep-data </dev/null >"$scratch/out" 2>&1 ||
    { show_output; fail "installed uninstall.sh failed"; }
[ ! -e "$HOME/.local/bin/clipsy" ] || fail "binary left behind"
[ -f "$HOME/.local/share/clipsy/history.db" ] || fail "history lost with default prefix"
[ -z "$(find "$HOME/.local/share/clipsy" -maxdepth 1 -name 'install-manifest' -o -maxdepth 1 -name 'version' -o -maxdepth 1 -name 'uninstall.sh' -o -maxdepth 1 -name 'created-dirs')" ] ||
    fail "install files left in the data folder"
[ ! -e "$HOME/.local/bin" ] || fail "empty ~/.local/bin created by the installer was left behind"
sh "$here/uninstall.sh" --delete-data </dev/null >"$scratch/out" 2>&1 && fail "uninstall should fail when nothing is installed"
rm -rf "$HOME/.local" "$HOME/.config"
ok "default prefix: ~/.local/share/clipsy/uninstall.sh works and keeps history"

# 8. A prefix with a space in it.
spaced="$scratch/my apps"
install_sh --deb "$deb" --prefix "$spaced" || { show_output; fail "install into a spaced prefix failed"; }
assert_installed "$spaced"
uninstall_sh --prefix "$spaced" --keep-data || { show_output; fail "uninstall of spaced prefix failed"; }
assert_empty "$spaced"
ok "works with a space in the prefix"

# 9. Bad input is refused before anything is written.
install_sh --deb "$deb" --prefix "relative/dir" && fail "relative prefix was accepted"
grep -q "absolute" "$scratch/out" || fail "no message for a relative prefix"
uninstall_sh --prefix "" && fail "empty prefix was accepted by uninstall"
[ ! -e relative ] || fail "created a relative folder"
ok "relative or empty prefixes are refused"

# 10. Missing GTK4/libadwaita: stop with the exact command to ask an admin for.
mkdir -p "$scratch/fakebin"
printf '#!/bin/sh\nexit 0\n' >"$scratch/fakebin/ldconfig"
chmod +x "$scratch/fakebin/ldconfig"
if PATH="$scratch/fakebin:$PATH" sh "$here/install.sh" --deb "$deb" --prefix "$scratch/nolibs" \
    </dev/null >"$scratch/out" 2>&1; then
    fail "installed although GTK4/libadwaita looked missing"
fi
grep -q "sudo apt install libgtk-4-1 libadwaita-1-0" "$scratch/out" || { show_output; fail "no admin command shown"; }
[ ! -e "$scratch/nolibs" ] || fail "wrote files although libraries were missing"
ok "missing GTK4/libadwaita stops with the admin command"

# 11. Download path against a local fake release: good checksum installs...
fake="$scratch/fake-release"
mkdir -p "$fake/download/v$deb_version"
cp "$deb" "$fake/download/v$deb_version/$deb_name"
(cd "$fake/download/v$deb_version" && sha256sum "$deb_name" >SHA256SUMS)
CLIPSY_RELEASES_BASE="file://$fake" install_sh --version "$deb_version" --prefix "$scratch/fake-ok" ||
    { show_output; fail "install from a fake release with a good checksum failed"; }
assert_installed "$scratch/fake-ok"
ok "download path verifies a good checksum and installs"

# ...and a wrong checksum aborts without installing anything.
printf '%064d  %s\n' 0 "$deb_name" >"$fake/download/v$deb_version/SHA256SUMS"
if CLIPSY_RELEASES_BASE="file://$fake" install_sh --version "$deb_version" --prefix "$scratch/fake-bad"; then
    fail "installed although the checksum was wrong"
fi
grep -q "does not match its checksum" "$scratch/out" || { show_output; fail "no checksum error shown"; }
[ ! -e "$scratch/fake-bad" ] || fail "wrote files although the checksum was wrong"
ok "a wrong checksum aborts and installs nothing"

# A SHA256SUMS without our file is refused too.
printf '%064d  %s\n' 0 "something-else.deb" >"$fake/download/v$deb_version/SHA256SUMS"
CLIPSY_RELEASES_BASE="file://$fake" install_sh --version "$deb_version" --prefix "$scratch/fake-bad" &&
    fail "installed although SHA256SUMS had no entry for the file"
[ ! -e "$scratch/fake-bad" ] || fail "wrote files although SHA256SUMS had no entry"
ok "a SHA256SUMS without the package's entry aborts"

# 12. The real thing: latest release from GitHub, run as a lone downloaded file.
if [ "${CLIPSY_TEST_OFFLINE:-0}" = "1" ]; then
    echo "skip real download (CLIPSY_TEST_OFFLINE=1)"
else
    mkdir -p "$scratch/dl"
    cp "$here/install.sh" "$scratch/dl/install.sh"
    if ! sh "$scratch/dl/install.sh" --prefix "$scratch/net" </dev/null >"$scratch/out" 2>&1; then
        show_output
        fail "installing the latest release from GitHub failed"
    fi
    grep -q "Checking the download" "$scratch/out" || fail "checksum step did not run"
    [ -x "$scratch/net/bin/clipsy" ] || fail "binary missing after network install"
    "$scratch/net/bin/clipsy" --version >/dev/null || fail "downloaded binary does not run"
    net_version="$(cat "$scratch/net/share/clipsy/version")"
    sh "$here/uninstall.sh" --prefix "$scratch/net" --keep-data </dev/null >"$scratch/out" 2>&1 ||
        { show_output; fail "uninstall after network install failed"; }
    assert_empty "$scratch/net"
    ok "downloads the latest release ($net_version) from GitHub, verifies it, installs and uninstalls"
fi

echo ""
echo "All $passed tests passed."
