# litehtml

Vendored from https://github.com/litehtml/litehtml at commit
`69254c75c415a093f89324eaadcd059a47d42c97` (2026-11-18): `include/`, `src/`
(with the bundled gumbo in `src/gumbo/`) and `LICENSE`. Built by `build.rs`.
See `docs/adr/0006-rendered-view-engine.md`.

Two changes, marked `Postal:` in the source:

- `document::rebuild_render_tree` is public in
  `include/litehtml/document.h`, so `cpp/cell_grid.cpp` can rebuild the boxes
  after it snaps the computed styles to whole cells.
- `render_item::split_inlines` in `src/render_item.cpp` recurses only into
  inline children. It went into inline-blocks too, so a link around an
  inline-block button with a block inside cut the button's box in two empty
  halves, one above its label and one below.

To update, replace these directories from a newer commit, apply the changes
again and update the hash.
