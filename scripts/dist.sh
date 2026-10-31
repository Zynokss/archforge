#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)
archive="packaging/archforge-${version}.tar.gz"
tar --transform="s,^,archforge-${version}/," -czf "$archive" Cargo.toml Cargo.lock src data assets scripts/dist.sh scripts/install.sh packaging/PKGBUILD run.sh README.md LICENSE
sha256sum "$archive"
echo "Source archive ready. Build the local Arch package with: cd packaging && makepkg -si"
