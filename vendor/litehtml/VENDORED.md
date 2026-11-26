# litehtml

Vendored from https://github.com/litehtml/litehtml at commit
`69254c75c415a093f89324eaadcd059a47d42c97` (2026-11-18): `include/`, `src/`
(with the bundled gumbo in `src/gumbo/`) and `LICENSE`. Built by `build.rs`.
See `docs/adr/0006-rendered-view-engine.md`.

Eight changes, marked `Postal:` in the source:

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
- `string_id` in `include/litehtml/string_id.h` has `int` as its fixed
  underlying type. Names interned while parsing get ids past the last
  enumerator, out of the range of an enum without one: undefined behaviour
  that UBSan reports on most mail.
- `text_align` in `include/litehtml/css_values.h` has the `-webkit-left`,
  `-webkit-right` and `-webkit-center` values Blink maps an `align`
  attribute and `<center>` to (`html_tag::map_align_attribute`,
  `master_css.h`). They lay out as left, right and centre
  (`css_properties::get_text_align`), and a table does not inherit them
  (`css_properties::compute`): text in a table under `<td align=center>`
  stood centred where a browser sets it left.
- `el_text::set_text` in `include/litehtml/el_text.h` replaces a word's
  text after parsing and measures it again, so `cpp/superscripts.cpp` can
  show the digits of a `<sup>` raised.
- `document::create_node` in `src/document.cpp` and
  `document_container::split_text` in `src/document_container.cpp` make one
  `el_space` of a run of spaces, not one per space. Each element holds its
  own styles, a few KB: a mail indented for its tables had 45,000 of them,
  235 MB to parse. Line feeds and tabs stay one element each.

To update, replace these directories from a newer commit, apply the changes
again and update the hash.
