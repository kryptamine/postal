# The Message List holds every Thread's key and reads its cards for the window

The Message List keeps one small row per Thread of the shown Folder in
memory, its key and what orders it, and reads what a card shows (senders,
subject, Snippet, flags, count) from the Cache only for the Threads in the
window around the screen. An `Update::Inbox` rebuilds the rows from the
Cache in one grouped query and reads the window's cards again; nothing is
merged in memory, so the Cache stays the one truth.

## Why

Until now every `Update::Inbox`, `Folders` with a Backfill and `Paged`
read the whole Folder into `Message` values, its Threads found elsewhere
with it, and grouped them into a fresh `Listing`. The cost grows with the
Folder, not with the change. Measured on the reference machine (ADR 0001),
release build, 10,000 Messages in the Inbox through the fake IMAP server:

| What | Count | Worst |
| --- | --- | --- |
| applying one `Update` (`arrive`) during a Backfill to 10,000 | 210 | 25 ms |
| of which the Cache read (`Cache::inbox`, no limit) | 212 | 19–27 ms |
| of which building the `Listing` | 210 | 3 ms |
| a sync write (`state.rs` Backfill) | 19 | 11 ms |

The main query itself takes about 2 ms; the query for Threads found
elsewhere about 10 ms even when it returns nothing; the rest is building
10,000 `Message` values with five owned strings each. At 50,000 Messages a
single Update would cost around 100 ms. Every Message held costs a few
hundred bytes, so a Folder of 100,000 alone would pass the 30 MB RSS
budget. Postal targets any user's mailbox, not the maintainer's.

## The design

- **Rows.** `Listing` becomes one row per Thread, newest first: its `Key`
  and its newest Message's arrival and body key, a few dozen bytes. They
  come from one `GROUP BY` over the shown Folder's `message` rows, on an
  index that serves it; in All Inboxes over every Account's Inbox at once,
  as the Accounts share one database (ADR 0014's edge stays).
- **Cards.** The window is the rows on screen with a margin of a few
  screens either side. Its cards are read in one query by Thread key and
  aggregated in Rust. A card is answered when any Message of its Thread
  is, in the shown Folder or another, so the query reads that flag from
  the Thread's Messages elsewhere too. Scrolling inside the margin touches no Cache;
  crossing it reads the next window into the same slots, reusing their
  buffers, so the scroll hot path stays free of allocations (ADR 0001).
- **The open Thread.** Its stack, with the Messages found in other
  Folders, is read when the Preview or Reading shows it, not for every
  Thread on every Update.
- **Positions.** The rows are positional, so the selection, `keep` across
  an Update, the scroll offset and "n of total" work as they do now. A
  Thread is found again by a scan of the keys, which costs well under a
  millisecond at 100,000.
- **Updates.** `Update::Inbox` stays "something changed"; `mail` is
  untouched. Rebuilding the rows and the window is cheap enough that no
  delta is needed.
- **Search** keeps its own whole-cache corpus for now: a separate change.

Targets, at 100,000 Messages in one Folder on the reference machine:
an `Update::Inbox` to its frame within 16 ms, and the Message List's
memory within 5 MB.

## Considered options

- **Merge deltas into an in-memory list.** `mail` would send what changed
  and the screen would merge it. The cost per Update becomes the change's,
  but the whole Folder stays in memory, past the RSS budget, and every kind
  of write (insert, expunge, flags, Snippets, moves and Undo, Threads
  elsewhere, a UIDVALIDITY reset) needs merge logic that must agree with
  the Cache. A list that silently drifts from the Cache is the worst bug a
  mail client can have. Thunderbird, which updates its view rows in place,
  still rebuilds the thread structure from its database in some cases "to
  avoid array desynchronization".
- **A keyset window only.** Read pages of Threads by key, with nothing for
  the whole Folder in memory. Least memory, but positions, "n of total" and
  jumping to the end need counting queries, and the selection must anchor
  the window. Web clients do this, with the server counting.
- **Coalesce Updates and tune the queries.** Cheap, and it raises the point
  where the budget breaks by perhaps two to three times, but the cost still
  grows with the Folder and the memory with it.
- **A thread of its own for the Cache.** It would free the event loop from
  the sync's writes, which mostly fit the budget already; the reads behind
  the Updates are the screen's own and would still block it.

Mature clients converge on these rows and cards. Thunderbird's
`nsMsgDBView` keeps `m_keys`, `m_flags` and `m_levels` for the whole view
and reads each row's header from the message database when the row is
drawn. aerc keeps every UID of the Folder and fetches the headers of the
visible rows alone. mutt holds every header in memory, and is known to be
slow to open a large mailbox.
