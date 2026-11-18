#!/usr/bin/env bash
# Builds the release .deb for clipsy-app.
# Needs cargo-deb: cargo install cargo-deb --locked
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v cargo-deb >/dev/null 2>&1; then
    echo "cargo-deb is not installed. Run: cargo install cargo-deb --locked" >&2
    exit 1
fi

cargo deb -p clipsy-app

deb="$(ls -t target/debian/clipsy_*.deb | head -n 1)"

# Add the GNOME Shell extension (Wayland support) to the package.
ext_uuid="clipsy@venkyeswar.github.io"
ext_src="gnome-extension/$ext_uuid"
for f in metadata.json extension.js; do
    if [ ! -f "$ext_src/$f" ]; then
        echo "Missing $ext_src/$f" >&2
        exit 1
    fi
done

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
dpkg-deb -R "$deb" "$stage/pkg"

ext_rel="usr/share/gnome-shell/extensions/$ext_uuid"
install -d "$stage/pkg/$ext_rel"
for f in metadata.json extension.js; do
    install -m644 "$ext_src/$f" "$stage/pkg/$ext_rel/$f"
    if [ -f "$stage/pkg/DEBIAN/md5sums" ]; then
        printf '%s  %s\n' "$(md5sum < "$ext_src/$f" | cut -d' ' -f1)" "$ext_rel/$f" \
            >> "$stage/pkg/DEBIAN/md5sums"
    fi
done
dpkg-deb --root-owner-group --build "$stage/pkg" "$deb" >/dev/null

echo "Built: $deb ($(du -h "$deb" | cut -f1))"
