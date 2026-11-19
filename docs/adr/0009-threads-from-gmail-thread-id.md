# Threads are grouped by X-GM-THRID, else each Message stands alone

The Message List shows one row per Thread. Gmail already decides which
Messages form a conversation and names it with `X-GM-THRID`, the same in
every Folder. Postal asks for it beside `X-GM-MSGID` in the first fetch phase
when the server lists `X-GM-EXT-1`, keeps it in the Cache, and groups the
shown Folder's cached Messages by it. So a row holds the Thread's Messages
that are in the shown Folder and in the Cache; a reply filed elsewhere, or
older than every cached row, is counted once the Thread has been read, as
below.

Reading a Thread asks the sync connection for the rest of it: IDLE ends
with `DONE`, Postal examines the Folder `LIST` marks `\All`, sends
`UID SEARCH X-GM-THRID <id>`, fetches the rows, whole bodies and Snippets
of what it finds, then selects and resyncs the shown Folder and idles on it
again (ADR 0008). Gmail keeps Trash and Spam out of All Mail, so their
Messages never join, and a Thread read in either is not looked up.

Those Messages are cached apart from every Folder's rows, keyed by
`X-GM-MSGID` with their Thread, and replaced each time the Thread is read.
A row's count and Reading's stack add those not in the shown Folder whose
body is cached. They have no UID in the shown Folder, so Reading tells its
Messages apart by body key, and their bodies come only with the Thread.

A server without Gmail's extensions would answer `BAD` to the item, as for
`X-GM-MSGID` (ADR 0005). There each Message is a Thread of its own.

async-imap parses `X-GM-THRID` but gives no accessor for it, so the list
fetch reads its responses by hand, as the whole-body fetch already did.

The Cache format changed with the new column. Postal has no users yet, so an
older Cache is dropped and resynced rather than migrated.

## Considered options

- **Threading by `References` and `In-Reply-To` (JWZ).** Works on any server,
  but needs more headers per Message, and Gmail's own grouping, which users
  see in its web client, would disagree with Postal's at the edges.
- **The IMAP `THREAD` extension (RFC 5256).** Gmail does not offer it.
- **Fetching `X-GM-THRID` in a second command.** No library change needed,
  but one more round trip on every fetch of new rows.
- **Caching the rest of a Thread as rows of All Mail.** No new table, but
  the resync of All Mail counts on its cached UIDs being its newest, which a
  few old Messages of one Thread would break.
- **Fetching the rest on the connection for bodies.** No IDLE to break, but
  that connection serves one Folder's UIDs at a time, and the Preview's
  bodies would wait behind a search of All Mail.
