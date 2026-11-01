# Loadout catalog

The catalog contains 659 packages in 17 categories: 621 from official Arch repositories and 38 selected community recipes. Nothing is selected or installed automatically.

Package availability was checked on **1 November 2026** against Arch's core, extra, and multilib databases for x86_64 and the AUR RPC service. Official packages are preferred. Community entries were selected from established projects with a current maintainer, no out-of-date flag at the check date, and inspected PKGBUILD upstream source locations. Personal packages, custom distribution bundles, debug packages, unavailable names, and unreviewed community candidates were excluded. Stable packages are generally preferred to development builds; RPCS3 is explicitly labeled as a source development build.

This is source and availability checking, not a security certification. AUR recipes are community-maintained; review the current PKGBUILD and transaction before installing. The source badge opens the package's Arch or AUR page. Repository membership and popularity do not guarantee safety, suitability, or availability on every architecture. Package names and maintainers can change after the snapshot.

## Ordering

- **Recommended:** curated common choices first, followed by more options and specialist tools. Within each group, higher pkgstats usage comes first; ties use the app name.
- **Popular first:** descending package presence in the October 2026 pkgstats reports, then name. Packages without a figure follow measured packages.
- **Name A–Z / Z–A:** alphabetical by display name.
- **Installed first:** locally installed packages first, then recommended order. Local installation state is not published or sent to a ranking service.

The usage snapshot covers **34,496 voluntary reports**. It measures package presence, including dependencies, rather than app launches, quality, or all Arch users. Hardware-specific software and new applications can be underrepresented. The common/more-options/specialist grouping is editorial curation, not a community vote. Hover an app title for its group and available survey percentage.

The catalog works offline. It does not query a popularity service when opened or upload the local package list. The snapshot is bundled with the app rather than presented as a live ranking.

## AI and large toolchains

The AI & ML category includes terminal coding assistants, Ollama and its accelerators, LM Studio, llama.cpp, Whisper.cpp, Jupyter, and scientific Python tools. Models, sign-in, paid service access, GPU drivers, and service activation are separate where applicable. GPU accelerators require compatible hardware. Ollama's accelerator packages depend on the base Ollama package; they are not treated as conflicting alternatives.

Known conflicting selections for LibreOffice, Wine, PyTorch, TensorFlow, and Code/Visual Studio Code are rejected before generating a bulk install command. Pacman and the AUR helper still perform their own transaction checks. Installing a service package does not configure an entire server or desktop session.

## Data files

- `catalog.tsv` supplies the app names, categories, descriptions, upstream icon hints, source, recommendation group, and survey percentages in basis points. Blank usage means no figure in this snapshot; 100 basis points is 1%.
- `catalog-metadata.json` records public package pages, upstream URLs, checked versions, community maintainer handles and recipe hashes, and usage provenance. It contains no local package inventory or personal configuration.

Sources: [Arch packages](https://archlinux.org/packages/), [AUR](https://aur.archlinux.org/), [pkgstats methodology](https://pkgstats.archlinux.de/methodology), and [October 2026 package statistics](https://pkgstats.archlinux.de/api/packages?startMonth=202610&endMonth=202610).

Application artwork and package-mark fallbacks are documented in [the icon notices](../app-icons/NOTICE.md). The app uses bundled artwork rather than relying on the active desktop icon theme.
