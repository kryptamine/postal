# Bodies are cached by X-GM-MSGID, else by UIDVALIDITY and UID

A Message's raw body (`BODY.PEEK[]`) is fetched once and kept in the Cache, so
a Preview shows from disk on every later run. The key must outlive what a
Folder's rows do not: a UIDVALIDITY change drops every row of that Folder and
refetches them, and the bodies should stay. On Gmail the same Message sits in
several Folders, and its body is fetched once for all of them.

Gmail gives each Message an `X-GM-MSGID` that never changes, whatever the
folder or UIDVALIDITY. Postal asks for it in the first fetch phase when the
server lists `X-GM-EXT-1`, and keys the body on it. A server without Gmail's
extensions would answer `BAD` to the item, so there the key is UIDVALIDITY in
the high 32 bits and the UID in the low ones: still unique, and a UIDVALIDITY
change only orphans the old bodies until the LRU evicts them.

Both keys are 64-bit unsigned and stored as their bit pattern in a signed
`INTEGER`; nothing sorts or compares them, so the sign does not matter.
Neither is unique across Accounts, so a body is cached under its Account and
its key together.

The raw bodies live in a byte-bounded LRU of about 200 MB, ordered by last
use; rendered Previews live apart in memory (ADR 0001).

## Considered options

- **The UID alone.** Simple, but a UIDVALIDITY change would refetch every body
  for Messages Postal already holds.
- **Always asking for `X-GM-MSGID`.** Postal's target is Gmail, but one `BAD`
  on another server would stop the whole sync.
