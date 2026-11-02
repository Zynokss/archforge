#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

if (( EUID == 0 )); then
  echo 'Run ./scripts/install.sh as your regular user, without sudo.' >&2
  exit 1
fi
if ! [[ -f /etc/arch-release ]]; then
  echo 'Arch Forge supports Arch Linux only.' >&2
  exit 1
fi
package_file=PKGBUILD
if [[ "${1:-}" == --cli ]]; then
  package_file=PKGBUILD-cli
elif [[ $# -gt 0 ]]; then
  echo 'Usage: ./scripts/install.sh [--cli]' >&2
  exit 1
fi
for tool in cargo makepkg fakeroot; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo 'Install the build tools first: sudo pacman -Syu --needed base-devel rust git' >&2
    exit 1
  fi
done

./scripts/dist.sh
(
  cd packaging
  makepkg --force --syncdeps --install --cleanbuild -p "$package_file"
)
if [[ "$package_file" == PKGBUILD-cli ]]; then
  echo 'Arch Forge terminal is installed. Run archforge-cli.'
else
  echo 'Arch Forge is installed. Use the application launcher, or run archforge-cli in a terminal.'
fi
