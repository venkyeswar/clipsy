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
echo "Built: $deb ($(du -h "$deb" | cut -f1))"
