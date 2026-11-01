<p align="center">
  <img src="assets/readme-banner.svg" alt="Arch Forge — system tools for Arch Linux" width="100%">
</p>

A native desktop utility for Arch Linux, built with Rust and GTK4.

Arch Forge brings system readings, gaming tools, package information, and maintenance commands into one place. Commands run as your desktop user; actions that need administrator access ask for authentication separately.

## Features

- **Overview** — live CPU, memory, and storage graphs, hardware details, uptime, temperature, and failed-service status.
- **Gaming** — installation checks and setup guidance for Steam, GameMode, MangoHud, Vulkan tools, Gamescope, and Lutris.
- **Cleanup** — package-cache usage and previews, journal usage, orphan-package inspection, and reviewed cleanup actions.
- **Packages** — search and sort installed packages, inspect foreign packages, and check repository updates.
- **Diagnostics** — failed services, boot timing, Vulkan information, and a system report with copyable output.
- **App updates** — automatic version checks and one-click installation of verified GitHub release packages.

## Screenshots

![System overview in the Crimson theme](assets/screenshots/overview.png)

## Install

On **Arch Linux x86_64**, install the latest prebuilt release with:

```sh
curl -fLO https://github.com/Zynokss/archforge/releases/latest/download/archforge-x86_64.pkg.tar.zst
sudo pacman -U ./archforge-x86_64.pkg.tar.zst
```

This installs Arch Forge system-wide, including its **application-menu shortcut and red icon**. Open your application launcher, search for **Arch Forge**, and launch it. You only need the terminal for installation; Rust and a source checkout are not required.

You can also download the `.pkg.tar.zst` from the [latest release](https://github.com/Zynokss/archforge/releases/latest) and install the downloaded file with `sudo pacman -U /path/to/package.pkg.tar.zst`. If your desktop has an Arch package installer, you can open the package with that instead.

Arch Forge requires GTK4 4.20 or newer and a desktop polkit authentication agent for administrator actions. Run the app as your regular desktop user. It refuses to run as root or on a distribution whose `/etc/os-release` does not identify it as Arch Linux.

### Install from source

For aarch64, or if you prefer to build locally:

```sh
sudo pacman -Syu --needed base-devel rust git
git clone https://github.com/Zynokss/archforge.git
cd archforge
./scripts/install.sh
```

If you already cloned the repository, run `git pull --ff-only` followed by `./scripts/install.sh` inside it. The installer builds an Arch package and uses pacman to install the app, its runtime dependencies, launcher, and icons. Run it as your regular user; pacman will ask for administrator authentication when needed.

### Updates

Arch Forge checks for stable releases in the background when it opens. Choose **App updates** in the sidebar to see the running app version, release notes, and a manual check. When a newer version is available, the sidebar shows **Update available**.

Click **Update to …** to download and install it. The app checks the package's SHA-256 digest against GitHub's release metadata and verifies its name, version, and architecture before starting pacman. Your desktop asks for administrator authentication. Pacman handles dependencies and file ownership; an unsuccessful download or checksum check never starts installation. Arch Forge restarts automatically after installation, waiting for any running in-app cleanup or maintenance operation to finish. A failed restart offers a retry button.

Updates require an internet connection, `curl`, and a desktop polkit agent. Downloaded packages come from this project's GitHub releases and are not separately signed. Automatic installation is available only when a package for your architecture is attached to the release; aarch64 users can build from source. Checking does not install anything until you click Update.

### Package availability

Arch Forge is distributed through GitHub Releases. It is not currently published in the AUR or Arch's official repositories, so `pacman -S archforge` and AUR-helper installation are not available. The package installed above is tracked by pacman and can be removed with `sudo pacman -Rns archforge`.

If you installed an older `arch-forge` package, run the installation command above once and accept pacman's replacement prompt. The package and executable are now named `archforge`; the application-menu name remains Arch Forge.

### Development

To run a checkout without installing it, install the build dependencies and use:

```sh
sudo pacman -Syu --needed rust gtk4 pkgconf pciutils polkit curl
cargo run --release --locked
```

`cargo run` only builds and opens the app. It does not install an application-menu entry, icon, or system binary. Use one of the installation methods above for normal desktop use.

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

Cleanup always starts with a review. Package-cache cleanup keeps the latest two cached versions; a separate action removes all archives for uninstalled packages. Journal cleanup removes archived files older than seven days. Each system cleanup first loads a read-only preview before enabling authentication. Orphan dependencies can be inspected, then removed through a reviewed interactive pacman transaction in a terminal. Empty lists never start a removal command.

Protected pacman download folders can prevent a complete unprivileged size check. Arch Forge marks that result as partial and offers an authenticated, read-only measurement.

Package setup offers **Open terminal**: the command appears at an editable prompt and waits for Enter before running. The setup dialog closes after the terminal opens successfully. Startup failures leave the dialog and copy action available. The terminal stays open afterward so you can read the output. Arch Forge uses `xdg-terminal-exec` when available, then supported installed terminals such as Kitty, Foot, Alacritty, Konsole, or GNOME Terminal. `TERMINAL` can specify a supported executable name or path, without extra arguments. If no terminal is found, the copy action remains available. Steam launch options are copied into Steam instead. Update checks cover configured repositories, not the AUR. Package changes are detected after pacman finishes, and the current page and package search are refreshed without reopening the app. Existing tool executables are detected even if their expected package entry is missing, so the app offers configuration instead of a conflicting install.

Personal cleanup offers home Trash, yay build cache, pip cache, and thumbnails. Paths come from the current account and `XDG_DATA_HOME` / `XDG_CACHE_HOME`, with standard home-directory defaults; pip also honors `PIP_CACHE_DIR`. Relative XDG paths are ignored. No username is embedded in the application. Trash cleanup covers the home Trash's files and metadata, not Trash directories on other mounted volumes.

Personal cleanup previews selected entries and requires a separate permanent-clean action. It runs as your regular user, includes hidden entries, keeps the cache root folders, and refuses root/home directories or linked cleanup roots. Deletion uses retained directory descriptors, does not follow symlinks, and does not traverse other filesystems. Changed root folders require another review. Close apps using a cache first; active apps can recreate cache files. A separate `yay -Sc --aur` action opens a terminal and lets yay ask which AUR cache entries to remove, without running yay as root or hiding errors.

Command dialogs keep the full output visible beside a summary of the result. The output scrolls independently, with colors for warnings, errors, and completion messages; action buttons stay visible. Summaries show reported values such as cache size, cleanup candidates, failed units, startup duration, and Vulkan device information. The Diagnostics page also shows key findings beside the log, including affected services, the slowest startup unit, GPU and driver details, and report usage figures. Missing fields stay marked as not reported. Empty package-cache previews show zero candidates and disable cleanup; commands and complete output remain available to inspect and copy.

## Appearance

Choose Crimson, Sage, Lavender, Sand, or Desktop colors from the sidebar. The selection is saved in `~/.config/archforge/appearance`, respecting `XDG_CONFIG_HOME`.

Desktop colors can follow an existing end4/Quickshell palette at `~/.local/state/quickshell/user/generated/colors.json`, respecting `XDG_STATE_HOME`. If that palette is unavailable, the app uses Sage. Arch Forge does not modify the shell configuration.

## License

[MIT](LICENSE). Arch Forge is an independent project and is not affiliated with Arch Linux.


### Tools and navigation

Use **Tools** to search by task (for example, `missing files`, `boot`, `DNS`, or `cache`), or use the category tabs. The grouped view starts with short previews for each category; **View all** opens the complete category. Search returns all matching tools, with no preview limit. Sort by recommended order or name, or switch to a compact list. Every tool has a built-in outline icon, independent of the desktop icon theme. **Ctrl+K** opens tool search; **Ctrl+1–7** switch between Overview, Tools, Gaming, Cleanup, Packages, Diagnostics, and Loadouts. Read-only tools include package file checks, package-list export, file ownership, package history, drives, mounts, free space, enabled services, timers, boot errors, boot timing, SSD trim status, network addresses, DNS status, and power profiles. Output can be copied or saved; system changes open an editable command in your terminal and wait for Enter.

**Package cache location** changes where pacman downloads package archives. Installed files keep the locations defined by their packages; pacman's `RootDir` is intended for another installation or chroot. Arch Forge inserts a managed first `CacheDir` in `[options]`, preserves existing entries as fallbacks, validates the proposed configuration with `pacman-conf`, and backs up `/etc/pacman.conf` before an atomic replacement. Existing archives are kept. The cache measurement and paccache tools follow the configured cache directories.

The cache folder must use a mounted Linux filesystem and protected, root-owned parent directories; symlinks, group/other-writable paths, and sensitive system trees are refused. The parent must exist; the final cache folder can be created with mode 0755. Removable drives must be mounted before use. **Restore previous settings** removes only Arch Forge's managed entry, preserves other edits, and creates another backup. Applying either action requires the installed current version of Arch Forge and an explicit `sudo` command in your terminal. No configuration changes happen merely by opening the setting.

References: [pacman configuration](https://man.archlinux.org/man/pacman.conf.5.en), [pacman-conf](https://man.archlinux.org/man/pacman-conf.8.en), [paccache](https://man.archlinux.org/man/paccache.8.en).


### Installing apps on another drive

Open **Tools → Storage → App install locations**. Flatpak mode can register a mounted Linux data filesystem, then install an application by its Flatpak ID into that named location. Refresh after completing the setup command. The command adds Flathub to the selected installation and retains Flatpak's interactive transaction review. Existing apps are not moved. App data stays in the home directory, and temporary downloads or metadata can still need system space. Sign out and back in if shortcuts from a newly added installation are missing.

This setup accepts persistent, root-owned mounts under `/mnt`, `/media`, or `/home` using ext4, Btrfs, XFS, or F2FS with execution enabled. The root filesystem and filesystems sharing its backing device are excluded. The administrator-reviewed helper creates a dedicated folder and a new configuration file without replacing existing installations. Its filesystem UUID and mount point are recorded; installation commands check both before invoking Flatpak. Drives must stay mounted to run their apps. The current installed Arch Forge version is required for these helper commands.

**Pacman: system package storage** shows where `/usr` is mounted and how much root space remains. Pacman packages use the paths specified by their packages. Growing that storage requires expanding or migrating a filesystem, or a layout-specific mount plan; this feature does not move system directories, edit `fstab`, or provide per-package drive selection. Cache relocation is a separate setting for downloaded archives.

Reference: [Flatpak custom installations](https://docs.flatpak.org/en/latest/tips-and-tricks.html#adding-a-custom-installation).


### Loadouts: bring a package selection to a fresh Arch install

Open **Loadouts** to choose from 659 curated packages across 17 categories, including desktop apps, terminal tools, office and study, graphics and CAD, backup, networking, virtualization, accessibility, and AI/ML. The catalog includes 621 official repository packages and 38 selected AUR recipes. Search spans package names and descriptions. Source and status filters, pagination, **Select matches**, and exact-name additions keep larger selections manageable. Category navigation scrolls independently; compact windows use a category dropdown. **Recommended** places curated common choices before more options and specialist tools, using an October 2026 pkgstats snapshot within each group. **Popular first** uses those survey percentages directly; alphabetical and installed-first sorting are also available. See [catalog sources and selection rules](data/loadouts/CATALOG.md) for the scope and limits of these rankings. Selections stay in place while package database updates refresh the other pages.

- **Capture this system** reads explicitly installed repository and foreign package names, plus enabled repository names for reference (without saving URLs or keys). Configuration folders are optional and unchecked by default. Preview the snapshot, then save its JSON file to another drive or your own backup storage.
- **Save selection** exports the packages and checked dotfiles currently in the workspace. **Import profile** loads a saved selection without installing or applying anything.
- **Package status** distinguishes installed versions, cached archives, missing packages, and repository updates. Status filters help narrow the list. **Skip installed packages** is enabled by default; saved profiles and exported scripts still retain the full selection for another installation. Cached archives may be older; pacman verifies and reuses suitable files.
- **Check for updates** uses `checkupdates` from pacman-contrib, which refreshes a separate database. It does not check AUR packages. **Update system** reviews a full repository upgrade rather than a partial package upgrade.
- **Uninstall** on a package, or **Uninstall selected**, first runs a read-only dependency preview. If another app needs a selected package, the dialog lists the blockers without opening a terminal. Otherwise it opens a reviewed `pacman -R` command. Pacman checks dependencies and asks for confirmation; recursive dependency removal and automatic acceptance are not enabled. Installed/cache status refreshes periodically while the package list is visible.
- **Install missing** opens a reviewed command in your terminal. Repository packages use `pacman -Syu --needed`; AUR packages use an already installed `paru` or `yay` as your regular user. **Export install script** creates a standalone package-only script. Neither flow silently accepts transactions or bootstraps an AUR helper.
- Foreign packages can be local builds, so they are not silently treated as AUR packages. The optional foreign-to-AUR checkbox is an explicit source decision. Custom repositories and multilib must already be configured where needed.

Desktop kits cover Hyprland, KDE Plasma, GNOME, Xfce, Cinnamon, and Sway. Two starter color bundles, **Amber Night** and **Crimson Night**, include Kitty and GTK settings; Hyprland and Sway also get compositor and Waybar starters; Plasma gets a KDE color scheme and matching color settings. Hyprland's starter uses the current Lua configuration format. GNOME, Xfce, and Cinnamon kits install session packages and shared app colors, leaving desktop-wide appearance to their own settings. No display manager or service is enabled automatically.

**Review dotfiles** compares checked text files with their current contents. Apply is a separate action: replaced files are backed up under `$XDG_STATE_HOME/archforge/loadouts/backups` (normally `~/.local/state/archforge/loadouts/backups`) before per-file atomic writes. Backups and exported profiles are private files. Symlinked paths, paths outside the allowed configuration roots, oversized bundles, and files changed since preview are refused. Home-directory references captured in text are represented by a portable placeholder and resolved for the destination account.

The backup restores replaced files; newly created files are listed in its metadata for manual removal when undoing a profile. If a later write fails, the output identifies the partial result and backup. Configurations can contain startup commands and private values: review imported files before applying or sharing them. Writing desktop configs may trigger the desktop's own file watchers; Arch Forge does not send reload, focus, or workspace-switch commands.

A loadout is not a disk image. Partitions, accounts, boot setup, repository configuration, services, drivers that need hardware-specific setup, and personal documents require their own backup or setup plan. Keep your exported loadout outside the installation you may lose.

The image-inspired **Amber Night** app palette is available under **Appearance**. It uses amber accents with cool charcoal surfaces; selecting it does not change your desktop's configuration.

### Application artwork

Loadouts ships with selected [Papirus](https://github.com/PapirusDevelopmentTeam/papirus-icon-theme) and [Simple Icons](https://github.com/simple-icons/simple-icons) artwork, so app logos are available even before the apps are installed. CLI tools and packages without bundled artwork get colored package marks instead of generic or missing-image icons. The artwork is installed separately from the executable; no icon theme or runtime download is required. Source code remains MIT licensed; artwork licenses and attribution are listed in [the icon notices](data/app-icons/NOTICE.md).
