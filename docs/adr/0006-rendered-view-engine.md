# HTML mail is laid out by litehtml, in a process of its own

The rendered View shows an HTML Message the way a mail client lays it out:
table columns, coloured blocks, buttons, borders, text styles. It needs an
HTML and CSS layout engine that Postal can feed its own font metrics, since
every glyph is one terminal cell. The text View (`html2text`) stays beside it,
one key away.

## Decision

[litehtml](https://github.com/litehtml/litehtml) (BSD-3-Clause, with gumbo,
Apache-2.0) is vendored in `vendor/litehtml/` and compiled by `build.rs` with
`cc`. A C++ container in `cpp/` supplies the metrics and draws into a flat
buffer of cells: `cell_grid.cpp` draws, `snap.cpp` snaps the styles before
layout, `image_labels.cpp` decides each image and `link_names.cpp` names a
bare picture link, `picture_boxes.cpp` fits or drops the boxes round
pictures, `buttons.cpp` and `titles.cpp` collapse buttons and cover bands
with `collapse.cpp`, `contents.cpp` gathers what each element shows for
those passes in one walk, and `colour.cpp`, `text.cpp` and `dom.cpp` hold
the colour, cell-width and element helpers.

- Every glyph is one cell of 8×16 CSS px, two for a wide one, and every font
  is one row high. A 600 px mail is 75 columns. `font-size` never changes the
  cell: large text is bold, and under 4 px it takes no room, which keeps the
  `font-size:0` whitespace trick of MJML and Outlook layouts working.
- Before layout, vertical margins, paddings and heights snap to whole rows,
  horizontal ones to whole columns, and a shown border is one cell thick;
  an `<hr>` keeps only its top border, or four would box nothing in. Under a
  shown border, vertical padding short of a row is dropped: the border is a
  row of its own already, and a 12 px padding made a button five rows high. A
  border the colour of the fill it sits on goes, as a browser cannot show it,
  unless that fill is page-light or grey and may itself give way to the
  terminal, when the border is the box's only shape. A transparent border
  takes no room either. Half
  a row rounds up above a box and down below it, so the 8 px under a paragraph
  and the 8 px over the next make one row. Collapsed table borders become
  separate ones with no spacing: litehtml gives each cell half a collapsed
  border, which puts the rule between two rows.
- Backgrounds fill the cells whose centre lies in the box; text rows use the
  same rule. A box's own background stops inside its border
  (`background-clip: padding-box`), as a 1px border looks; a cell thick, it
  would ring the line in the fill. Borders are drawn as `─ │ ┌ ┐ └ ┘` after
  the text, on cells without text.
- An icon label that is all its link shows takes its whole `alt`, up to 16
  columns, and a column after it: cut, `Insta…` named nothing. After layout,
  a grown label that spills past a block around it or past the pane goes
  back to the icon's width, with every grown one in its table, and the
  document is laid out again.
- Images are never fetched: remote ones are a tracking risk and add little to
  reading. Before layout each image either goes (`display: none`) or becomes
  a label one row high, its `alt` cut with `…` to the image's declared width.
  An empty box would only push the text apart: a 600×300 banner was 19 blank
  rows, a 24 px status icon a second row per table row. An image goes when:
  - it is under one column wide or 2 px high (a tracking pixel or spacer);
  - it has no `alt`, unless it is all a link shows, which keeps a label so
    Hint mode can still open it: the words of the last path segment made of
    lowercase words (`/e/watch-party-tickets-2004` reads `[watch party
    tickets]`), or else the host's domain without its subdomain, as a
    tracking redirect's path names nothing (`[revolut.com]`);
  - it is an icon, no side over 64 px, beside other text: it decorates that
    text, and its `alt` rarely fits its box (`GitHub` in 32 px read `GitH`);
  - its `alt` is already in the nearest text around it, within its table row
    and under 200 bytes, as an avatar's caption or a status icon's job name
    is (`Appfire` beside `Appfire · Sofia` read `Appfir Appfire`).

  - it is all a link shows and a link with words opens the same page (the
    href before its query, where mail tags where in the mail a link sits),
    or the next words in the mail are a link's: a product's picture beside
    its title, a banner over its button;
  - its `alt` is 16 columns or more and the mail shows it as text anywhere:
    a card's title under its picture, however far the markup puts it.

  A box with pictures and no text, a space even, was sized for them: its
  height and min-height go, or a product photo's cell stood twelve rows
  high. A spacer image (no `alt`, no link) keeps its box's gap.

  The outermost box round a gone picture that shows nothing else, no word
  and no label, goes too: its padding and fill framed only the picture, and
  a purple hero stood as an empty frame four rows high. A cell stays, or its
  row's columns would shift, and so does a row of a table with `rowspan`.
  A column whose cells below the header all show nothing goes whole, with
  its header, in a table without spans: the status icons of a CI report
  repeat each job's name. A text header over a single row may caption it
  instead, so it needs two such rows; a border that ruled the column off
  its neighbour at the table's edge goes with it.

  Each label gets a `src` of its own, since mail reuses one icon file under
  many `alt`s. A percentage width stays, so a long `alt` cannot widen its
  table past the pane.

- A button is the innermost box with a fill that is not page-light round
  one line of at most 40 columns, all of it a link's. It loses its vertical
  padding, height and borders: padding was rows of fill, and a slab three
  rows high read as no button. A page-light button gives way to the
  terminal, and the frame round its rows (below) is its shape.
- A filled box holding an `h1`, at most two lines of a 600 px mail (150
  cells), no link and no shown picture, in a colour no other box outside it
  fills, is a cover band its logo and pictures left: it loses its fill,
  borders, vertical padding, height and the vertical margins inside it, and
  its title stands as bold lines. A colour other boxes share is a section
  built of many boxes, or one of the mail's cards.
- An inline-block holding blocks is a column of a fluid layout and aligns
  to the top: on the baseline the columns' bottoms line up, which only
  images of one height kept level.

Four changes to litehtml, all recorded in `vendor/litehtml/VENDORED.md`.
`document::rebuild_render_tree` is public: the boxes are built from the
computed styles inside `createFromString`, before the snap, and must be built
again after it. A block inside an inline-block no longer splits the inline
around it, a litehtml bug that cut link-wrapped buttons in two. And a cell's
`colspan` and `rowspan` are clamped as HTML parses them: `colspan="0"` read
memory outside the table's columns. And `text-decoration: none` no longer
shifts by -1.

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
(`src/mail_colours.rs`): a background lighter than 0.6 luminance is the page
and becomes the terminal's own; a grey section too close to the terminal's
background becomes the terminal's too, or it would be moved into a grey slab,
while a coloured one, or a box small enough to be a control, moves away from
it, so a dark button stays visible on a dark terminal; text
too dark for the page's ordinary ink becomes the terminal's fg, other text
moves until it reads (WCAG 4.5:1); a grey border on the terminal's background
is drawn dim like Postal's own. The layout marks a background from a box four
rows high or half the pane wide as a section. It reads the page and grey
thresholds too, so they are part of the grid's format in `cpp/cell_grid.h`
and `src/cell_grid.rs`. `NO_COLOR` keeps the layout and drops every
colour. No 256- or 16-colour fallback: the target is a 24-bit terminal.

### Frames for section fills

A section's fill is a slab of colour its text has to be moved against, and
it reads differently on a light and a dark terminal. After layout, the fill
of a section goes and a frame in its colour is drawn round it, so the text
sits on the terminal's own background (`src/outlines.rs`, `src/region.rs`).
The pass works on the grid, so the layout, and what fits the pane, does not
change.

- A section fill is one not page-light, or a page-light one at least 10
  apart in some channel from the page: the colour most of the pane's two
  side columns lie on, white where the mail sets none. Cards fill the
  middle and may outnumber the page there. A light card laid on the page
  gave way to the terminal with it, and a mail's cards ran together.
- Section cells of one colour that touch are one region with one frame,
  however many boxes made them: mail builds a block out of many cells.
- The frame is drawn in the blank cells along the region's edge. An edge
  with text on it moves one cell out, where every cell there is blank and
  has no fill. Where it cannot, a level edge is not drawn, and an upright
  one leaves a gap at the text.
- A region as wide as the pane and four fifths of the mail high is the page
  of a dark mail: its fill goes and it gets no frame. So does one in the
  page's colour against one side of the pane, or against both when it is
  no rectangle: the page's margins round a narrower card, which a wide pane
  framed on each side, or filled where they wrap it top and bottom too.
- A region with the mail's own border along most of its edge gets no
  second one.
- The fill stays on a control, on a region under three cells high or wide,
  which has no room for a frame, and on a region that is not a rectangle:
  that is a drawing made of fills, as the bars of a chart are.

On 819 cached mails at 74 columns this changed 191, left the count that do
not fit at 63, and cut the filled cells to a fifth. The rules for picture
links, picture boxes, buttons, fluid columns and light cards then changed
504 of them, left 63 not fitting, and cut the `[link]` labels from 462 to
192. Dropping the boxes and columns a gone picture leaves empty then
changed 247 of them, left 63 not fitting, and lost no word but the `Status`
header of CI reports. Naming picture links by their address changed 93,
and left 10 `[link]` labels, on links with no http address. Cover bands
changed 7.

Deciding a section by the size of the merged region, not of each box, was
tried: it framed chart bars and footer gutters and improved nothing.

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
