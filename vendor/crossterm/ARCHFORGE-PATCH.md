# Local Crossterm patch

Source: crates.io `crossterm` 0.28.1 (MIT; see LICENSE).
Keep this copy until an upstream release fixes
https://github.com/crossterm-rs/crossterm/issues/1110.

The Unix Mio reader retries EOF and unexpected read errors forever. This can
consume a core after terminal disconnection and prevent ArchForge's main loop
from checking its signal shutdown flag. In `src/event/source/unix/mio.rs`, return
`UnexpectedEof` for a zero-byte read and propagate other unexpected read errors.
All other upstream source is unchanged.

The root Cargo patch applies to both ArchForge binaries and Ratatui's dependency.
`scripts/dist.sh` includes this directory in source packages.
