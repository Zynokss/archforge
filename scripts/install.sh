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
for tool in cargo makepkg fakeroot pkgconf; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo 'Install the build tools first: sudo pacman -Syu --needed base-devel rust git' >&2
    exit 1
  fi
done

./scripts/dist.sh
(
  cd packaging
  makepkg --syncdeps --install --cleanbuild
)
echo 'Arch Forge is installed. Open your application launcher and search for Arch Forge.'
