# Converting email HTML to clean terminal text

Research for [#4](https://github.com/kryptamine/postal/issues/4), under the
inbox MVP map [#1](https://github.com/kryptamine/postal/issues/1).

Settled before this research: Postal always renders the HTML part through its
own converter, and shows `text/plain` only when no HTML part exists. Links
become numbered footnotes. Allocation and latency matter.

Researched 2026-11-17. Crate versions are the latest on crates.io that day.

## Question

How does Postal turn a Message's HTML part into a Preview that is clean and
loses no content? That splits into two layers:

1. **MIME**: find the HTML part, undo the transfer encoding (base64,
   quoted-printable) and decode the charset to UTF-8.
2. **HTML to text**: parse the HTML, drop what a mail client would not show,
   lay out what remains at the terminal width, and number the links.

## Candidates

| Layer | Crate / tool | Version | Notes |
|---|---|---|---|
| MIME | [`mail-parser`](https://github.com/stalwartlabs/mail-parser) | 1.0.1 | Stalwart Labs. Zero-copy, lazy decoding, MSRV 1.98 |
| MIME | [`mailparse`](https://github.com/staktrace/mailparse) | 0.18.0 | Eager `String` bodies, charsets via the `charset` crate |
| HTML to text | [`html2text`](https://github.com/jugglerchris/rust-html2text) | 0.17.4 | html5ever DOM, table layout, CSS subset, footnotes. MSRV 1.85 |
| HTML parse/rewrite | [`lol_html`](https://github.com/cloudflare/lol-html) | 3.0.3 | Cloudflare's streaming rewriter. Tokenizer plus CSS selectors, no tree |
| HTML parse | [`html5ever`](https://github.com/servo/html5ever) | 0.40.1 | Servo's WHATWG parser. A custom converter would sit on this |
| External | w3m / lynx `-dump` | n/a | What mutt/neomutt users wire in through mailcap |

## Method

The scratch experiment (not committed) was a release-build Rust binary with a
counting global allocator. For each message it ran every step 200 times (10
for the large one) and recorded mean time, allocation count, bytes allocated
and peak live heap. Machine: Apple Silicon laptop, Rust 1.94.

**Corpus.** Downloading the public template repositories (Postmark, Mailchimp
blueprints) was blocked in this session, and Postal has no mail corpus of its
own yet. So the corpus is **synthetic** but follows real patterns, one
message per pattern:

| File | Pattern reproduced |
|---|---|
| `newsletter.eml` | Mailchimp-style: QP, `<style>` with media queries, MSO conditional comments, class-hidden and inline-hidden preheaders padded with `&zwnj;&nbsp;`, 600px nested layout tables, two-column article grid, table-wrapped CTA buttons, two tracking pixels, unsubscribe footer |
| `newsletter-large.eml` | The same with 400 articles, base64: 534 KB message, 395 KB HTML |
| `receipt.eml` | Postmark "receipt" template shape: layout tables, a real data table, button, URL fallback, footer |
| `gmail-reply.eml` | Gmail `div.gmail_quote` with a nested `blockquote` |
| `outlook-reply.eml` | Outlook: windows-1252, QP, `MsoNormal`, `<o:p>&nbsp;</o:p>` spacer paragraphs, `#divRplyFwdMsg` header with no blockquote |
| `github-notification.eml` | GitHub notification footer: beacon `<img>`, an all-properties hidden `<span>` |
| `iso-2022-jp.eml` | Legacy CJK charset |
| `html-only.eml` | A single `text/html` part, content hidden by `font-size:0`/`opacity:0` and by `visibility:hidden` |

**This does not replace an evaluation on real mail.** That is the open item
at the end.

## Findings: MIME layer

### mail-parser

- Conforms to RFC 5322 and RFC 2045–2049, and returns the RFC 8621 body
  lists (text, HTML, attachments)
  ([README](https://github.com/stalwartlabs/mail-parser#readme)).
- 1.0 is **lazy and zero-copy**: "Nothing is transfer-decoded or converted
  from its character set during the parse", and "practically all strings
  returned by this library are references to the input raw message"
  (README). `body_html()` returns `Cow<str>`. `MessagePart::decoded()` stays
  borrowed when nothing needs decoding, and `decode_into(&mut Vec<u8>)`
  reuses a caller's buffer (`src/view/part.rs`).
- Charsets: every WHATWG label plus UTF-7, IBM850 and the ISO-8859 and
  windows-125x families are built in. Shift_JIS, EUC-JP, ISO-2022-JP, GBK,
  BIG5 and similar need the `full_encoding` feature, which pulls in
  `encoding_rs` (README, "Supported Character Sets").
- SIMD boundary scanning, SIMD base64/QP (`encodify`), `simdutf8`. Fuzzed and
  tested under MIRI. Stalwart Mail Server runs it in production (README).
- **MSRV 1.98** (`Cargo.toml`). Current stable is 1.100, so it builds.
  The machine here had 1.94, so the experiment ran 0.11.9, which decodes
  eagerly. The 1.0 numbers should come out at or below those shown.
- It also converts HTML to text itself, when a part has no text
  alternative. Postal should not use that: the settled rule is our converter.

### mailparse

- `ParsedMail<'a>` borrows the raw bytes, but `get_body()` always returns an
  owned `String`, and `get_body_encoded()` lowercases the
  Content-Transfer-Encoding into a new `String` on every call
  (`src/lib.rs`, lines 741–818).
- Charsets go through the `charset` crate (`encoding_rs` plus UTF-7).

### Measured (0.11.9 vs 0.18.0, extract the HTML part as `String`)

| Message | mailparse | mail-parser |
|---|---|---|
| github-notification (1.7 KB) | 4.3 µs, 55 allocs | 3.8 µs, 32 allocs |
| newsletter (10.8 KB, QP) | 24 µs, 77 allocs, 80 KB | 22 µs, 38 allocs, 55 KB |
| newsletter-large (534 KB, base64) | 367 µs, 55 allocs, 1.3 MB | 805 µs, 32 allocs, 1.6 MB |

- Both decoded every message the same way, windows-1252 and ISO-2022-JP
  included. The only difference was CRLF vs LF in the decoded output, which
  does not matter to HTML.
- Both are negligible next to the HTML step (below). The base64 gap on the
  large message comes from 0.11's eager decode, which 1.0 removed.

**MIME verdict: `mail-parser` 1.x with `full_encoding`.** It allocates
fewer times, borrows by default, is lazy for attachments Postal never opens,
and has the stronger conformance and fuzzing record. One gotcha: a part with
no `charset` parameter but a `<meta charset>` in its HTML is decoded per the
MIME default. Re-check `<meta>` only if real mail shows mojibake.

## Findings: HTML-to-text layer

### Option A: `html2text` as the layout engine

What the source shows (`src/lib.rs`, `src/css.rs`,
`src/render/text_renderer.rs`):

- It parses with **html5ever**, the WHATWG tree builder, so mis-nested
  mail HTML is repaired the way browsers repair it.
- The pipeline is split: `Config::parse_html` → `RcDom`,
  `dom_to_render_tree` → `RenderTree` (`Clone`),
  `render_to_lines(tree, width)` → `Vec<TaggedLine<Annotation>>`. Postal can
  **parse once and re-render on terminal resize**.
- `TextDecorator` is a public trait (link start/end, image, emphasis,
  quote prefix, `finalise(urls)` for footnote lines), so Postal can own the
  footnote format and attach annotations for ratatui styling. `rich()` gives
  `RichAnnotation::Link(url)`, `Image(src)`, `Colour` and others.
- `link_footnotes(true)` appends `[n]` after each link and passes the URL
  list to `finalise`. The numbering is per link *occurrence*, done by the
  renderer, with no deduplication.
- With the `css` feature, `use_doc_css()` honours `display:none` from
  `<style>` and `style=""`, and treats `height:0`/`max-height:0` together
  with `overflow:hidden` as hidden. It does **not** handle
  `visibility:hidden`, `opacity:0` or `font-size:0`.
- `empty_img_mode(IgnoreEmpty)` (the default) drops `<img>` with no alt, so
  most spacer GIFs and tracking pixels vanish on their own. `no_table_borders()`,
  `raw_mode()` (tables as one column), `allow_width_overflow()` (otherwise
  narrow widths return `Err(TooNarrow)`).
- The changelog shows table edge cases recently fixed to stop panics
  (rowspan/colspan, `rowspan=0`, deep nesting overflow, CSS infinite loop in
  0.17.4). Treat it as code that can panic on hostile input.

Measured, `plain().link_footnotes(true)`, width 80:

| Message | html2text | + `use_doc_css` |
|---|---|---|
| gmail-reply (559 B HTML) | 30 µs, 495 allocs, peak 35 KB | 32 µs |
| receipt (1.6 KB) | 116 µs, 1.6 k allocs, peak 96 KB | 119 µs |
| newsletter (8.5 KB) | 390 µs, 4.4 k allocs, 0.95 MB total, peak 273 KB | 425 µs |
| newsletter-large (395 KB) | 19.6 ms, 236 k allocs, 49 MB total, peak 14 MB | 21.2 ms |

Output quality:

- **Good with no extra work**: Gmail quotes become nested `> ` blocks,
  `<pre>` and `<code>` survive, Japanese text is intact, Outlook's
  `– € “”` decode correctly, the receipt's data table lines up, and the
  GitHub hidden span and both preheaders are gone (CSS on).
- **Without `use_doc_css`** the preheader leaks: "Five things you missed
  this week" followed by 40 `&zwnj;&nbsp;` pairs. CSS is required.
- **Layout tables are the main defect.** The 600px newsletter renders as a
  box-drawn two-column grid of about 40-column cells, and narrow cells wrap
  "Read more →" across two lines. The receipt's wrapper tables draw lines
  across the full width.
- `font-size:0`/`opacity:0` and `visibility:hidden` content leaks.
- Outlook's `<o:p>&nbsp;</o:p>` paragraphs leave lines holding only spaces.
- The built-in decorators write Markdown-ish `[text][n]` and `# heading`.
  Postal's own decorator replaces that.
- Footnotes repeat: an article whose image and headline share one URL gets
  two numbers.

### Option B: `lol_html` as the whole converter

A ~60-line prototype (hide `head/style/script/[style*=display:none]`, newline
on block tags, `[n]` after `a[href]`) ran about **6× faster** with about
**8× fewer allocations** (newsletter-large: 3.2 ms, 29 k allocs, peak 633 KB).
But that is because it does much less:

- lol_html is a **tokenizer with selector matching, not a tree builder**. It
  does not repair mis-nested markup.
- Text chunks are raw source: `&rarr;` came out as `&rarr;`. Entity
  decoding, whitespace collapsing, wrapping, lists, `<pre>`, blockquote
  prefixes and table layout would all be ours to write.
- It cannot look ahead. Whether a table holds `<th>` (data) or nested
  tables (layout) is unknown when its start tag is seen.
- `on_end_tag` handlers must be `'static`, which forces `Rc<RefCell<_>>`
  state.

A full converter on lol_html means writing a text layout engine. Not worth
it for the MVP.

### Option C: lol_html pre-pass, then html2text

A lol_html pass that removes hidden elements (`display:none`,
`visibility:hidden`, `opacity:0`, `font-size:0`, `max-height:0`,
`mso-hide:all`, `[hidden]`), 1×1 images and `script/template`, and renames
"layout" tables to `div`, matched by selectors (`table[role=presentation]`,
`width="100%"|600|570|640`):

- It fixed the newsletter: one column, no box drawing, CTA on one line,
  hidden content gone, footnotes intact.
- It **broke the receipt**: the `width="100%"` purchase table was flattened
  into one cell per line. A selector cannot tell a layout table from a data
  table, because the answer depends on descendants.
- Cost: about 35 µs fixed per call (about 1 k allocations, mostly compiling
  selectors on every call; lol_html accepts pre-parsed `Cow<Selector>` to
  avoid that). 3.1 ms on the large message. It also serialises and reparses
  the HTML.

Conclusion: the hidden-content and pixel removal is right, but it belongs in
a **DOM pass**, where the layout-table decision can see the whole subtree.

### Option D: external w3m / lynx

`lynx -dump` lists links as numbered "References" at the end; the numbered
footnote convention comes from it. Mutt-family clients pipe HTML through
these tools via mailcap `copiousoutput`. For Postal this means a runtime
dependency the user must install, a process spawn per Message, and no
annotations for styling. Rejected. Neither tool was on this machine, so they
were not measured.

### DOM pass on html2text's tree: what is possible

`html2text::RcDom` and `Handle` are public. `Node.children` is
`RefCell<Vec<Handle>>` and `attrs` is `RefCell<Vec<Attribute>>`, so nodes
can be **removed** and attributes **rewritten** between `parse_html` and
`dom_to_render_tree`. The element *name* is immutable, and the `Node` type
is not exported, so a layout table **cannot be renamed** to `div` in place.
Its cells' children can still be hoisted into the table's parent, which
makes the table disappear. Hoisting has to put a block boundary between
cells; reusing each `td`'s child list in order is enough when cells hold
block content, as email cells do.

## Edge-case policy

| Case | Policy | Mechanism |
|---|---|---|
| Hidden preheader | Drop | `use_doc_css` handles `display:none`. The DOM pass adds `visibility:hidden`, `opacity:0`, `font-size:0`, `mso-hide:all`, `[hidden]`. Then strip leftover runs of U+200C/U+034F/U+00AD/NBSP |
| Tracking pixel | Drop | `IgnoreEmpty` drops alt-less images. The DOM pass also drops `<img>` with `width`/`height` ≤ 1 or `display:none` |
| Content image | `[alt]`, linked when inside `<a>` | Decorator `decorate_image`. `cid:` sources keep only the alt |
| Layout table | Flatten to blocks | DOM pass: a table is layout if it has `role=presentation`, contains a table, or has no `<th>` and only one row or one column. Otherwise keep it as a data table and render it with `no_table_borders()` |
| Data table | Keep columns | html2text table layout. It stacks columns vertically when they do not fit |
| Link | `text[n]`, footnote list at the end | Own `TextDecorator` holding shared `Rc<RefCell<FootnoteState>>` (sub-block decorators are separate instances), deduplicating by URL. Skip the marker when link text equals the URL. `mailto:` keeps the address |
| Buttons (table-wrapped `<a>`) | Plain link | Falls out of flattening |
| Quoted reply | Keep, as `> ` | Blockquotes already render so. Outlook `#divRplyFwdMsg` has no blockquote and shows unquoted. Folding quotes is post-MVP. Candidate markers: `.gmail_quote`, `blockquote[type=cite]`, `#divRplyFwdMsg`, `#appendonsend`, `.moz-cite-prefix`, `.yahoo_quoted` |
| Footer / unsubscribe | Keep | It is content, and the unsubscribe link must stay reachable |
| MSO conditional comments | Drop | Comments are not rendered |
| Whitespace-only lines | Collapse | Post-pass: trim trailing space, squeeze blank runs to one |
| Converter panic or error | Fall back | `catch_unwind` around render; fall back to `text/plain` if present, else tag-stripped text |

## Allocation profile and where it goes

- MIME is microseconds and tens of allocations. Not a concern.
- html2text dominates: about 0.4 ms and 1 MB of short-lived allocations for
  a typical newsletter, 20 ms and a 14 MB peak for a 395 KB outlier. That is
  well under a frame at typical sizes, but should not run on every keypress.
  - Render once per (Message, width). Keep the `RenderTree` (it is `Clone`)
    for resizes. Cache the rendered lines for the selected Message.
  - Render off the UI thread so an outlier never blocks input.
  - Cap the input (for example, render only the first N MB of HTML and say
    so) to bound the worst case.
- The extra DOM pass is a linear walk over a tree that already exists, with
  no reparse. Option C cost about 20–30% because it reparsed; a DOM pass
  should cost far less.

## Recommendation

1. **MIME: `mail-parser` 1.x** with `full_encoding`. Take
   `body_html(0)` as `Cow<str>`; use `body_text(0)` only when no HTML part
   exists. Do not use its built-in HTML-to-text.
2. **HTML to text: build Postal's converter on `html2text` 0.17** with the
   `css` feature, through its split pipeline: `parse_html` → Postal DOM pass
   → `dom_to_render_tree` → `render_to_lines(width)`, configured with
   `use_doc_css()`, `link_footnotes(false)`, `no_table_borders()` and
   `allow_width_overflow()`.
3. **Postal owns two small pieces**:
   - a **DOM pass** that drops hidden content and tracking pixels, and
     flattens layout tables by hoisting cell children (the rules are in the
     table above);
   - a **`TextDecorator`** that numbers links `[n]` with deduplication,
     emits the footnote list in `finalise`, and annotates spans for ratatui.

   Add a small text post-pass: zero-width runs and blank-line squeezing.
4. **Reject** a converter built entirely on lol_html (it means writing a
   layout engine), and external w3m/lynx (runtime dependency, no styling).
5. **Guard** the converter with `catch_unwind`, a size cap, a background
   thread and a per-width cache.

Open before implementation (#11): run this pipeline over a **real corpus**,
for example 50–100 Messages exported from the developer's Gmail (newsletters,
receipts, notifications, Outlook threads), and keep anonymised fixtures for
the integration tests. The layout-table heuristic and the hidden-style list
are the parts most likely to need tuning.
