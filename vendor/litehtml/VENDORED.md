# litehtml

Vendored from https://github.com/litehtml/litehtml at commit
`69254c75c415a093f89324eaadcd059a47d42c97` (2026-11-18): `include/`, `src/`
(with the bundled gumbo in `src/gumbo/`) and `LICENSE`. Built by `build.rs`.
See `docs/adr/0006-rendered-view-engine.md`.

One change, marked `Postal:` in the source: `document::rebuild_render_tree` is
public in `include/litehtml/document.h`, so `cpp/cell_grid.cpp` can rebuild
the boxes after it snaps the computed styles to whole cells.

To update, replace these directories from a newer commit, apply the change
again and update the hash.
