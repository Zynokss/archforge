# Arch Forge

A native desktop utility for Arch Linux, built with Rust and GTK4.

Arch Forge brings system readings, gaming tools, package information, and maintenance commands into one place. Commands run as your desktop user; actions that need administrator access ask for authentication separately.

## Features

- **Overview** — live CPU, memory, and storage graphs, hardware details, uptime, temperature, and failed-service status.
- **Gaming** — installation checks and setup guidance for Steam, GameMode, MangoHud, Vulkan tools, Gamescope, and Lutris.
- **Cleanup** — package-cache usage and previews, journal usage, orphan-package inspection, and reviewed cleanup actions.
- **Packages** — search and sort installed packages, inspect foreign packages, and check repository updates.
- **Diagnostics** — failed services, boot timing, Vulkan information, and a system report with copyable output.

## Build and run

Requires Arch Linux, a Rust toolchain, GTK4 4.20 or newer, and `pkgconf`.

```sh
sudo pacman -Syu --needed rust gtk4 pkgconf pciutils polkit
git clone https://github.com/Zynokss/arch-forge.git
cd arch-forge
cargo run --release --locked
```

Run the app as your regular desktop user. It refuses to run as root or on a distribution whose `/etc/os-release` does not identify it as Arch Linux.

### Arch package

```sh
./scripts/dist.sh
cd packaging
makepkg -si
```

The included `PKGBUILD` builds from the local source archive produced by `dist.sh`. It installs the binary, application launcher, and icon. This is a local build recipe, not an AUR package.

### Optional tools

| Package | Used for |
| --- | --- |
| `pacman-contrib` | Update checks and package-cache maintenance |
| `vulkan-tools` | Vulkan diagnostics |
| `gamemode` | GameMode integration and self-check |
| `mangohud` | In-game performance overlay |
| `ttf-material-symbols-variable` | Rounded interface icons |

A desktop polkit authentication agent is required for administrator actions. Google Sans Flex is used when available; the interface falls back to Rubik or the system sans-serif font. Native symbolic icons are used when the Material font is absent.

## Maintenance

Cleanup always starts with a review. Package-cache cleanup keeps the latest three cached versions of each package. Journal cleanup removes archived files older than fourteen days. Orphan inspection lists packages without removing them.

Protected pacman download folders can prevent a complete unprivileged size check. Arch Forge marks that result as partial and offers an authenticated, read-only measurement.

Package setup provides a terminal command so you can review pacman's transaction. Update checks cover configured repositories, not the AUR. Installed-package and service information is read at startup; restart the app after making changes to refresh it.

## Appearance

Choose Crimson, Sage, Lavender, Sand, or Desktop colors from the sidebar. The selection is saved in `~/.config/arch-forge/appearance`, respecting `XDG_CONFIG_HOME`.

Desktop colors can follow an existing end4/Quickshell palette at `~/.local/state/quickshell/user/generated/colors.json`, respecting `XDG_STATE_HOME`. If that palette is unavailable, the app uses Sage. Arch Forge does not modify the shell configuration.

## License

[MIT](LICENSE). Arch Forge is an independent project and is not affiliated with Arch Linux.
