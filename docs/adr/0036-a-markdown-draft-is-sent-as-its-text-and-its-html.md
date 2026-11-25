# A Markdown Draft is sent as its text and its HTML, and Neovim parses only what the user wrote

A Draft is written in plain text, which a list, a link or a code block
reads poorly in a client that shows HTML (issue #197). With
`compose-format = "markdown"`, at the top level of `config.toml` or in an
`[[account]]`, Send builds a `multipart/alternative`: the body exactly as
written in `text/plain`, so a plain-text reader gets the readable source,
and its HTML in `text/html`. With files attached, the alternative is the
first part of a `multipart/mixed`. `plain`, the default, sends the one part
it always has.

## What is converted

Only what the user wrote. The body is cut where a Forward's original starts
(the line `forward_draft` writes above it), else where its trailing Quote
starts (`render::quoted_text::cut`, the line naming whom it quotes
included), and the signature is the last `-- ` line before that cut on.
The text above is CommonMark through comrak, with GFM tables, strikethrough
and autolinks. Everything after it is escaped into
`<pre style="white-space:pre-wrap">` blocks, each run of `>` lines inside as
many `<blockquote>`s as it is deep, `>>` and `> >` alike. Run through
Markdown, a signature's `-- ` under a line of text makes a setext heading
of it, and someone else's plain text turns its `*` and `1.` into emphasis
and lists it never meant.

- A soft line break is `<br>`: mail is written line by line.
- A line of the user's right under a `>` line they quote ends the quote:
  `CommonMark` would take it in as a lazy line, and an answer written
  between quoted lines would read as part of what it answers.
- Raw HTML in the source is escaped, never passed through, and comrak
  drops `javascript:` links.
- No `<style>` block and no remote resource. Inline styles go only where a
  client would drop something: a code block's monospace, a quote's bar,
  a table cell's padding and the rule under its heading. Borders on every
  cell were tried: litehtml separates collapsed borders, and each cell
  showed as a box of its own.

## In the TUI

The Draft's preview of a Markdown Draft lays its HTML out in the Rendered
View (ADR 0006) at the pane's width, kept until the width changes, and `v`
switches to the text as written. A layout that fails shows the text, and
so does an Outbox Message, built already in the format it was sent with.
Conversion never fails, so there is no plain-text fallback for Send.

## In Neovim

Postal tells the editor the Draft's format in `$POSTAL_COMPOSE_FORMAT`,
the Account placed as Send places it. The Draft format knows only To, Cc,
Subject and Attach, and an environment variable needs no new header and no
CLI call before the buffer is set up.

The plugin then sets the filetype to `mail.markdown`, registers the
`markdown` treesitter parser for it and restricts the parser's included
region to the rows Postal converts, cut the way `compose::markdown` cuts
them and recomputed on each change, so neither
a header such as `Subject: # x` nor the signature parses as Markdown.
The filetype is set before the rest of the Draft's set-up, as the
markdown ftplugin sources the HTML one, whose `omnifunc` would replace the
plugin's. Treesitter turns syntax off, and `syntax=mail` puts the headers'
colours back.

render-markdown.nvim attaches to any filetype with `markdown` among its
dotted parts and asks `get_parser(buf)` for the buffer's parser, which is
the restricted one: it renders a Markdown Draft with no setting of the
user's. Without it, Neovim's bundled Markdown highlights and conceals the
marks, with `conceallevel=2`.

## Considered options

- **pulldown-cmark.** Two crates under MIT, but no GFM autolinks: a bare
  URL would need a scanner of Postal's own. comrak has them, with hard
  breaks and escaping as options; it brought `BSD-2-Clause` and, through
  `finl_unicode`, `Unicode-DFS-2016` into `deny.toml`, both permissive.
- **The whole body as Markdown.** Simpler, but the signature and the
  quoted original are not Markdown, and read wrong as it.
- **render-markdown.nvim alone.** Nothing would show without it, and its
  public API attaches only by filetype, so the dotted filetype is needed
  either way.
- **A renderer of the plugin's own, by extmarks.** Headings, lists, code
  and tables drawn by hand: much Lua for what render-markdown already does.
- **A `:PostalPreview` of the HTML.** The TUI's preview shows it after the
  editor already; while writing, the Markdown itself is what is edited.
