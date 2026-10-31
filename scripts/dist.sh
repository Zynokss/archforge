#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
version=0.3.0
archive="packaging/arch-forge-${version}.tar.gz"
tar --transform="s,^,arch-forge-${version}/," -czf "$archive" Cargo.toml Cargo.lock src data assets README.md LICENSE
sha256sum "$archive"
echo "Source archive ready. Build the local Arch package with: cd packaging && makepkg -si"
