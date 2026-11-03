# Changelog

## 0.12.1

- Updated both bundled and standalone terminal editions with actual-installed-name/source filtering and identity-preserving loadout exports.
- Added per-workspace sorting (`S`), filter reset (`R`), focused package version/description details, and separate repository, foreign/local and AUR-recipe filters.
- Deduplicated installed variant rows, reused catalog metadata for exact-name additions/imports, and retained category identity across rescans.
- Invalidated stale update flags after installed versions change and cached variant lookups for large package lists.

## 0.12.0

- Redesigned Packages with summary cards, package descriptions and sizes, source/category filters, pagination, bulk selection and reviewed package actions.
- Unified package variant matching across desktop and terminal views, loadouts, gaming checks and icons. Common packaging suffixes retain catalog metadata while actions preserve actual installed names and explicit source decisions.
- Added Uncategorized and Reset filters, fixed variant category filtering, and prevented duplicate installed variants in package browsing.
- Overhauled every appearance palette with deep tinted surfaces, illuminated borders, hover glow and a matching Arch banner.
- Replaced the custom Arch mark with a transparent silhouette throughout the app and launcher icons, and fixed logo clipping in shorter headers.
- Verified all palettes, compact layouts, category/sort/pagination behavior and desktop workflows in isolated graphical sessions.

## 0.11.0

- Added a standalone terminal edition for Linux consoles, SSH and terminal emulators, with no GTK or VTE dependency.
- Built a full-screen Ratatui interface for Overview, Packages, Cleanup, Diagnostics, Build loadout, Capture loadout, Import loadout and Wiki.
- Added keyboard navigation, persistent pane state, categories, source/status filtering, bulk package selection and command previews.
- Added background scans, actual system readings, small-terminal layouts, `NO_COLOR` support and read-only plain-text reports.
- Added explicit command review, fresh authentication in a real PTY, searchable output, terminal clipboard support and private log exports.
- Shared installed-package variant detection, terminal-capable package catalog entries, offline Wiki content and desktop-compatible loadout profiles.
- Bundled the TUI with the full desktop package and added a separate terminal-only installer to releases.
- Updated documentation and both desktop and terminal screenshots.

## 0.10.0

- Added embedded command sessions with review and fresh authentication.
- Added an offline Wiki with 76 command references across 11 topics.
- Updated Overview, Settings, Cleanup, command output and Loadouts.
- Fixed installed-status detection for reviewed Vesktop, Lutris and GTK theme variants.

[Releases and downloads](https://github.com/zynoksy/archforge/releases)
