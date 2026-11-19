# Search matches every cached Folder

A Search used to match the shown Folder alone, so finding a Message meant
guessing its Folder first. It now matches the Messages of every Folder in the
Cache, except Trash and Spam, which Gmail's own search leaves out by default.

A Message Gmail files under several labels is cached once per Folder under one
`X-GM-MSGID`, its body key (ADR 0005), so the matches are deduplicated by body
key: the copy in the shown Folder wins, else the first in Folder Picker order.
Threads group across Folders by `X-GM-THRID`, as in Reading. On a server
without Gmail's extensions copies in two Folders have different keys and both
match.

A match from another Folder has its body fetched from that Folder. Ending the
Search always stays in the shown Folder: on the match when that Folder holds
its Thread, else back where `/` was pressed.

Only the Cache is searched: a Folder never opened, or the pages of one never
backfilled, are not found. Every keypress stays in process (ADR 0007); the
Cache is read whole at `/` and again with each change while a Search is on.

## Considered options

- **The server's search (`SEARCH`, Gmail's `X-GM-RAW`).** Complete, but not
  fuzzy, a round trip per Query, and nothing offline.
- **Syncing every Folder in the background.** Would make the Cache complete,
  at the cost of the sync connection's time, which goes to the shown Folder
  alone now; left for later.
