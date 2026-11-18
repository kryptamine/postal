# Threads are grouped by X-GM-THRID, else each Message stands alone

The Message List shows one row per Thread. Gmail already decides which
Messages form a conversation and names it with `X-GM-THRID`, the same in
every Folder. Postal asks for it beside `X-GM-MSGID` in the first fetch phase
when the server lists `X-GM-EXT-1`, keeps it in the Cache, and groups the
shown Folder's cached Messages by it. So a row holds only the Thread's
Messages that are in the shown Folder and in the Cache: a reply filed
elsewhere, or older than every cached row, is not counted until it is.

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
