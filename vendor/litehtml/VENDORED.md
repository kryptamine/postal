# litehtml

Vendored from https://github.com/litehtml/litehtml at commit
`69254c75c415a093f89324eaadcd059a47d42c97` (2026-11-18): `include/`, `src/`
(with the bundled gumbo in `src/gumbo/`) and `LICENSE`. Built by `build.rs`.
See `docs/adr/0006-rendered-view-engine.md`.

Four changes, marked `Postal:` in the source:

- `document::rebuild_render_tree` is public in
  `include/litehtml/document.h`, so `cpp/cell_grid.cpp` can rebuild the boxes
  after it snaps the computed styles to whole cells.
- `render_item::split_inlines` in `src/render_item.cpp` recurses only into
  inline children. It went into inline-blocks too, so a link around an
  inline-block button with a block inside cut the button's box in two empty
  halves, one above its label and one below.
- `table_grid::add_cell` in `src/table.cpp` clamps `colspan` to 1..1000 and
  `rowspan` to 1..65534, as HTML parses them. A `colspan="0"` made the cell
  span to the column before the first, a read outside the grid's columns
  that laid the table out differently from run to run.
- `style::parse_text_decoration_line` in `src/style.cpp` skips `none`, the
  value at index 0, which shifted 1 by -1: undefined behaviour on nearly
  every mail.

To update, replace these directories from a newer commit, apply the changes
again and update the hash.
