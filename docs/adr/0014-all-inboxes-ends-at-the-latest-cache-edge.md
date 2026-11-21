# All Inboxes ends at the latest cache edge

All Inboxes (issue #64) merges every Account's cached Inbox by arrival. Each
Account caches its newest 500 Messages, then older pages as a Backfill
fetches them, so the Accounts' caches end at different dates. Merged as they
are, the rows below the latest of those edges would show one Account's old
mail with none of another's from the same days, though the server holds it:
that Account would look empty there.

So All Inboxes shows rows only down to the latest of the Accounts' oldest
cached arrivals. Near the end of the list, the Backfill goes to the Account
whose edge stops the list. Its page lowers
that edge, and the next Account's edge, if later, stops the list next.

An Account whose server had nothing older than its oldest cached Message sets
no edge: a small Inbox, all of it cached, would otherwise stop the list for
good. Postal learns this two ways. A resync, and each IDLE update, stores the
Inbox's `EXISTS`; when the Cache holds at least that many of its UIDs, the
Account sets no edge from the first frame of the next run, before it has
signed in (issue #81). Otherwise a Backfill that finds nothing older says so,
once per run, and Postal forgets it when the Account's oldest cached UID
changes or a Backfill of another of its Folders answers. Until then such an
Account stops the list like any other, and the first Backfill near the end
asks it.

The stored count errs one way only. New mail since it was stored is newer
than the Cache, and an expunge the Cache has not seen leaves it holding more,
not fewer. A Cache holding fewer than the stored count leaves the answer to
the Backfill.

## Considered options

- **Merging every cached row.** Simple, and wrong in the way above.
- **Keeping whether each Inbox is cached to its first Message in the Cache.**
  Saves one Backfill per run, at the cost of a column that every resync,
  Backfill and expunge would have to keep true. The stored `EXISTS` gives the
  same answer from a count the resync already has, and nothing else writes it.
