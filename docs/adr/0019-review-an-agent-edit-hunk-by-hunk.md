# An agent's edit to a Draft is reviewed hunk by hunk, and a hunk over the user's own edit is a conflict

An agent's write to the Draft's body does not replace it. The plugin diffs
the write against the body the turn was sent with (`vim.diff`) and opens a
review: each hunk the agent changed is put in the buffer at once,
highlighted, with the lines it replaces shown over it as virtual lines.
The user accepts or rejects a hunk under the cursor, or every hunk left,
and the pane's top line counts what is left. Putting the clean hunks in at
once, as one undoable change, keeps the buffer showing the mail as it will
read; rejecting puts the old lines back.

The user may edit the Draft while the agent works. Their edits are a
second diff against the same sent body. An agent's hunk that overlaps or
merely touches a hunk of the user's, adjacent lines included, as git
counts them, joins it in one conflict: the buffer keeps the user's lines,
marked, with the agent's shown over them, and only an accept takes the
agent's, for the whole conflict, the user's lines in it included. A conflict where the user wrote what the agent did is no hunk at
all. Every other hunk is placed as usual.

A later write in the same turn replaces the open review, its placed hunks
taken back first; a read in that turn gets the agent's last write, so an
agent editing twice builds on its own text. An instruction given while a
review is open waits for it to end, so each prompt carries a body the
user has settled. A chat started again after its agent ended takes over
the review still open, and waits for it the same way.

The placed hunks are their own undo step. An undo past it ends the
review, the body left as the undo made it, since the hunks' marks would
no longer sit on the lines they placed.

## Considered options

- **The agent's lines as virtual lines, the buffer untouched until an
  accept.** The buffer would show the old mail while the review is open,
  and a hunk would only be readable in its virtual lines, which do not
  wrap or take a cursor.
- **A whole-body diff in a split (`:diffthis`).** No per-hunk accept
  beyond `do`/`dp`, and nothing to tell a conflict from a clean hunk.
- **A three-way merge of the user's edits into the agent's text.** It
  would apply a hunk the user never looked at over lines they were
  changing.
