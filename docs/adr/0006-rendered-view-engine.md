# HTML mail is laid out by litehtml, in a process of its own

The rendered View shows an HTML Message the way a mail client lays it out:
table columns, coloured blocks, buttons, borders, text styles. It needs an
HTML and CSS layout engine that Postal can feed its own font metrics, since
every glyph is one terminal cell. The text View (`html2text`) stays beside it,
one key away.

## Decision

[litehtml](https://github.com/litehtml/litehtml) (BSD-3-Clause, with gumbo,
Apache-2.0) is vendored in `vendor/litehtml/` and compiled by `build.rs` with
`cc`. A C++ container, `cpp/cell_grid.cpp`, supplies the metrics and draws into
a flat buffer of cells:

- Every glyph is one cell of 8×16 CSS px, two for a wide one, and every font
  is one row high. A 600 px mail is 75 columns. `font-size` never changes the
  cell: large text is bold, and under 4 px it takes no room, which keeps the
  `font-size:0` whitespace trick of MJML and Outlook layouts working.
- Before layout, vertical margins, paddings and heights snap to whole rows,
  horizontal ones to whole columns, and a shown border is one cell thick. Half
  a row rounds up above a box and down below it, so the 8 px under a paragraph
  and the 8 px over the next make one row. Collapsed table borders become
  separate ones with no spacing: litehtml gives each cell half a collapsed
  border, which puts the rule between two rows.
- Backgrounds fill the cells whose centre lies in the box; text rows use the
  same rule. Borders are drawn as `─ │ ┌ ┐ └ ┘` after the text, on cells
  without text.
- Images are never fetched: remote ones are a tracking risk and add little to
  reading. An image keeps the box its size gives, and shows its `alt`.

Two changes to litehtml, both recorded in `vendor/litehtml/VENDORED.md`.
`document::rebuild_render_tree` is public: the boxes are built from the
computed styles inside `createFromString`, before the snap, and must be built
again after it. And a block inside an inline-block no longer splits the
inline around it, a litehtml bug that cut link-wrapped buttons in two.

### A process of its own

`postal __lay-out <cols>` reads the HTML on stdin and writes the grid on
stdout (`src/cell_grid.rs` has the format); the render thread runs one per
Preview and kills it after 5 s. Two reasons:

- **Memory.** litehtml holds about 0.8 KB per byte of HTML while a document
  lives (one element per word, each over 3 KB), and the allocator keeps the
  pages after it is freed. In process, one 95 KB newsletter left the footprint
  27–44 MB higher, a 400 KB one up to 100 MB, against the 30 MB of ADR 0001.
  An exited process hands all of it back.
- **Hostile input.** Mail is untrusted, and litehtml is C++. A crash ends the
  layout process only; the Preview falls back to the text View.

### Colours

Mail colours assume a white page. Postal asks the terminal for its background
with OSC 11 at start (DA1 behind it, so a terminal that does not answer costs
no wait), takes a dark one when there is no answer, and moves the mail onto it
(`src/mail_colours.rs`): a background lighter than 0.8 luminance is the page
and becomes the terminal's own; a block too close to the terminal's background
moves away from it, so a dark button stays visible on a dark terminal; text
too dark for the page's ordinary ink becomes the terminal's fg, other text
moves until it reads (WCAG 4.5:1); a grey border on the terminal's background
is drawn dim like Postal's own. `NO_COLOR` keeps the layout and drops every
colour. No 256- or 16-colour fallback: the target is a 24-bit terminal.

## Measurements

Release build, Apple M2 Max, 76 columns, median of 10, spawn included (about
7 ms of it):

| Mail | HTML | Laid out |
|---|---|---|
| Bank statement fixture | 2 KB | 8 ms |
| Real newsletter | 95 KB | 43 ms |
| Synthetic newsletter | 391 KB | 171 ms |

All of it runs on the render thread, behind the placeholder, once per
(Message, width, View); drawing a laid out Preview allocates nothing. The
binary grows by 2.0 MB, a clean build by about 8 s of C++ on 12 cores, and
building needs a C++17 compiler.

## Considered options

- **blitz** (pure Rust: stylo, taffy, parley). Faster to parse and a quarter
  of the memory, but 164 crates, 13 MB of binary, 43 s of clean build, rustc
  1.91 above the MSRV, and the cell metrics only by rewriting computed font
  sizes behind the cascade's back, which any restyle undoes. It is pre-1.0,
  and Postal would lean on its internals: a port on every release.
- **litehtml in process.** Simplest, and 7 ms faster, but it breaks the RSS
  budget and lets a parser crash take the whole client down.
- **A dedicated malloc zone for litehtml, freed after each document.**
  litehtml interns names in a global table that grows during parsing, so
  freeing the zone would leave that table dangling.
