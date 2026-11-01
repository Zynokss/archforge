# Application icon artwork

Arch Forge's source code and native package marks are MIT licensed. The independent application artwork in `svg/` retains its upstream licenses. Names and marks identify third-party applications; they do not imply endorsement.

- **Papirus icon theme** — Papirus Development Team, GPL-3.0. Papirus is based on the Paper icon set by Sam Hewitt. The unmodified selected SVGs and full license are included here as separate artwork. Source: https://github.com/PapirusDevelopmentTeam/papirus-icon-theme
- **Simple Icons** — Simple Icons contributors, CC0-1.0 collection. This selection excludes entries with additional non-CC0 licenses recorded in upstream metadata. Colors have been adjusted for legibility on dark surfaces; paths are unchanged. The full collection license is included here. Source: https://github.com/simple-icons/simple-icons

`sources.json` records the exact upstream revisions, original file locations, and SHA256 hashes of the distributed files. `map.tsv` associates package names with these files. Related package variants may share the same application logo.

The artwork is installed separately under `/usr/share/archforge/app-icons`. No icon download or desktop icon theme is needed at runtime. Apps without bundled artwork, imported packages, and unavailable or unreadable assets use native colored package marks instead of a missing-image placeholder.
