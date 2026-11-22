# The Preview fetches a Message's header and text parts, not `BODY[]`

A body is fetched in two commands. `UID FETCH (UID BODYSTRUCTURE)` says
what each Message holds; then `BODY.PEEK[HEADER]`, each inline text part
(`BODY.PEEK[1.1]`) and the `.MIME` header of every part on the way to one.
Postal puts these back together as a smaller Message in MIME, with the
multiparts' own boundaries from `BODYSTRUCTURE`, and caches that. Messages
that ask for the same sections share one fetch.

## Why

`BODY[]` brings every attachment with the text. A 20 MB PDF made its
Preview wait on 20 MB, and the body then took a tenth of the 200 MB LRU
(ADR 0005), evicting dozens of other bodies to keep a file the Preview
never shows. The Preview, the pager, the editor's file, a Reply's Quote and
`postal thread show` all read only the text parts, and images are never
shown (the rendered View draws an alt in their place).

So the raw body in the Cache holds the text parts only, and an attachment
is never cached: opening it fetches its part on demand, and opening it
twice fetches it twice.

A text part sent as an attachment, any part that is not text, and an
attached Message are left out. A multipart that lists no boundary cannot
be put back, so a Message with one is fetched whole as before.

## Considered options

- **`BODY[]` under a size limit.** One command, and nothing to put back;
  but a Message just over the limit would have no Preview, and a limit
  under the size of a typical attachment leaves most attachment mail
  without one. It is the fallback if the part-wise body breaks rendering.
- **Caching the sections at the envelope pass.** It would save the
  `BODYSTRUCTURE` round trip, but the Cache would carry a description of
  each Message's MIME tree that only the body fetch reads.
