# Application icon artwork

Arch Forge's source code and the reusable functional symbols in `symbols/` are MIT licensed. The independent application artwork in `svg/` retains its upstream licenses. Names and marks identify third-party applications; they do not imply endorsement.

- **Papirus icon theme** — Papirus Development Team, GPL-3.0. Papirus is based on the Paper icon set by Sam Hewitt. The unmodified selected SVGs and full license are included here as separate artwork. Source: https://github.com/PapirusDevelopmentTeam/papirus-icon-theme
- **Simple Icons** — Simple Icons contributors, CC0-1.0 collection. This selection excludes entries with additional non-CC0 licenses recorded in upstream metadata. Colors have been adjusted for legibility on dark surfaces; paths are unchanged. The full collection license is included here. Source: https://github.com/simple-icons/simple-icons

`sources.json` records the exact upstream revisions, original file locations, and SHA256 hashes of the distributed application artwork. `map.tsv` associates package names with these files. Related package variants may share the same application logo. `package-symbols.tsv` maps packages without a recognizable logo to reusable function symbols; `category-symbols.tsv` provides a category fallback. The symbols share a muted, outline SVG style and are original Arch Forge artwork.

The artwork is installed separately under `/usr/share/archforge/app-icons`. No icon download or desktop icon theme is needed at runtime. Resolution prefers a recognizable application logo, then a package function symbol, then a category symbol, with a generic package outline as the final fallback.
