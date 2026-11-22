# A Forward attaches the parts it fetched through `Attach:` lines

A Forward (issue #110) sends the original's attachments with it. The
Cache never holds an attachment (ADR 0024), so a Forward fetches each part
from the server before its Draft opens, as a Reply waits on its bodies,
writes it under `forwarded/<draft>/` in the data directory beside
`drafts/`, and names each file on an `Attach:` line (issue #99).

The other way was a header of its own naming the original's parts, fetched
on Send. It would make Send wait on the network and the fetch connection,
and a Send from the Outbox could not build its `.eml` offline. With files
on disk, a Forward's Draft is any other Draft: the user drops an
attachment by deleting its line or adds one beside it, Send reads the files
into the Outbox once (ADR 0013), and a retry or a Send offline needs
nothing more.

The files stay as long as the Draft or its Message in the Outbox does,
since a refusal gives the Draft back with its `Attach:` lines. As the TUI
starts it removes each `forwarded/<draft>/` whose Draft is gone from
`drafts/` and `outbox/`: sent, or discarded and no longer undoable.

A Forward from a Message with attachments needs the fetch connection: signed
out, it says the Message is not downloaded rather than send it without them.
An attachment's type is guessed from its name on Send, as for any
`Attach:` line, not copied from the original.
