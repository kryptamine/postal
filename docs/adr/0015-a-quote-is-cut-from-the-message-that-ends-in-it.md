# A Quote is cut from the Message that ends in it

A Message that answers another usually repeats it, and that one repeats the
one before. In a Thread each of those is a Message of its own, so the Preview
and Reading showed the same mail many times over, and the few new words sat
above screens of old ones.

So the Preview and Reading leave out the Quote: the quoted mail a Message
ends in. One muted line stands for it, `z` shows it for that Message, and
`$PAGER` always shows the Message whole. The cut is made on the part's source,
before either View lays it out, so the rendered View, the text View and plain
text agree.

- **Only a Message that answers another**, one with `In-Reply-To` or
  `References`; Outlook sets the second alone. A forward or a digest quotes
  mail the Thread does not hold, and a newsletter may use `<blockquote>` for
  a pull quote.
- **Only at the end.** Quoted mail with words after it is an answer written
  under it, which makes no sense alone. Checked from the end, so several
  quotes with nothing between them go together.
- **Plain text**: the trailing `>` lines.
- **HTML**: the last `<blockquote>` inside no other. No client marks it the
  same way (`gmail_quote`, `type="cite"`, a bare style), and in a Message
  that answers another a trailing one is a quote whatever its class.
- **Outlook** wraps the quoted mail in no element. The Quote starts at the
  rule it draws above the quoted header (`border-top:solid #E1E1E1`, older
  `#B5C4DF`), else at the first block holding three bold labels ending in a
  colon and an address: `From:`, `Sent:`, `To:` in any language.
- **The line above** goes too when it names whom the quote is of: it ends in
  a colon, holds a digit or an `@`, and is at most 200 characters. That reads
  `On 4 Mar 2024, Bob <bob@example.com> wrote:` in any language, and leaves
  the writer's own `this is the part I mean:`. In plain text the line before
  it goes as well when the client broke one line in two: the next word
  would not have fit in 72 columns.
- **Never the whole body.** A Message that is nothing but its Quote is shown
  as it is.

Over the 97 cached Messages that answer another, this cut 42, each at the
writer's signature, and none of the rest held a trailing quote. Over the 367
others the same rules would have cut three: two Outlook answers and one
newsletter with three bold labels, which the first rule keeps whole.

## Considered options

- **Cutting by Thread size**, only where the Thread shows more than one
  Message. The common case is an answer in the Inbox to mail in Sent: one
  row, one Message, and the whole quote still shown.
- **Per-client markers only.** Misses every client not listed; the cached
  mail alone held three shapes, one with no marker at all.
- **Cutting in each View's own tree.** Two implementations, in Rust and C++,
  that would drift.
- **One switch for every Message**, like `v`. Showing one Quote in Reading
  would unfold them all and move the page.
