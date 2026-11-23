# Threads are grouped by X-GM-THRID, else by the root of References

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
`X-GM-MSGID` (ADR 0005). There the list fetch asks for `References` beside
`From` in `HEADER.FIELDS`, and a Message's Thread is its root: the first
`Message-ID` of `References`, else of `In-Reply-To` (from the `ENVELOPE`),
else its own. The Cache keeps the root hashed (FNV-1a, stable across builds)
in a column of its own, `root`, NULL on Gmail: `thread` still means a Gmail
Thread, whose Messages share a body key across Folders, and the flag and
move queries rely on that. A Message naming no `Message-ID` at all is a
Thread of its own.

The root is cheap, one header and no tree, and agrees with JWZ wherever the
`References` of every reply start at the same Message, as mail clients write
them. It splits a Thread whose replies dropped `References` and answer a
reply rather than the first Message; JWZ would join those through the chain.

Reading such a Thread shows what the shown Folder holds of it, with no
search on the server. The Account's own replies sit in Sent; looking them up
there by `UID SEARCH HEADER Message-ID` waits for a later change, as it
needs a cache of a Thread's other Messages keyed by something other than
`X-GM-MSGID`.

async-imap parses `X-GM-THRID` but gives no accessor for it, so the list
fetch reads its responses by hand, as the whole-body fetch already did.

The Cache format changed with the new column. Postal has no users yet, so an
older Cache is dropped and resynced rather than migrated.

## Considered options

- **Threading Gmail by `References` too.** One rule for every server, but
  Gmail's own grouping, which users see in its web client, would disagree
  with Postal's at the edges.
- **JWZ threading on other servers.** Joins replies that dropped
  `References`, but needs every Message's `In-Reply-To` chain walked, and a
  Thread's key would change as Messages arrive.
- **The IMAP `THREAD` extension (RFC 5256).** Gmail does not offer it.
- **Fetching `X-GM-THRID` in a second command.** No library change needed,
  but one more round trip on every fetch of new rows.
- **Caching the rest of a Thread as rows of All Mail.** No new table, but
  the resync of All Mail counts on its cached UIDs being its newest, which a
  few old Messages of one Thread would break.
- **Fetching the rest on the connection for bodies.** No IDLE to break, but
  that connection serves one Folder's UIDs at a time, and the Preview's
  bodies would wait behind a search of All Mail.
