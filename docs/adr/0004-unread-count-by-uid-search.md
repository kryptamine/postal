# The Unread count comes from `UID SEARCH UNSEEN`

The Folder Picker shows the shown Folder's Unread count, counted by the
server over the whole Folder, not over the fetched rows. Sync semantics (issue #14) chose `UID SEARCH RETURN (COUNT) UNSEEN`, which answers with one number.

`RETURN` makes the server answer with an `ESEARCH` response (RFC 4731), and
neither `imap-proto` 0.17, which async-imap parses with, nor `imap-types`,
which the fake server encodes with, knows it. async-imap treats an unparsable
line as a fatal stream error, so the command would break the session.

Postal sends plain `UID SEARCH UNSEEN` and counts the UIDs. The count is the
same; the answer grows with the number of Unread Messages, about 7 bytes each,
which stays off the frame path.

Resync's expunge check (issue #25) hits the same wall with
`UID SEARCH RETURN (ALL)`. Postal sends `UID SEARCH UID first:last` over the
cached range and diffs the plain list against the Cache. The list costs about
7 bytes per cached Message, and the search runs only when the server holds
fewer Messages in that range than the Cache does, which one `UID FETCH` of the
oldest cached UID tells.

## Considered options

- **`STATUS INBOX (UNSEEN)`.** One number, parsed by async-imap, but RFC 3501
  advises against `STATUS` on the selected mailbox.
- **Rewriting `ESEARCH` lines in the transport.** Keeps the one-number answer,
  at the cost of a hand-written parser hidden under the IMAP library.
- **Patching `imap-proto`.** The right fix long term; revisit when upstream
  parses `ESEARCH`, or when a large Unread count shows up in a measurement.
