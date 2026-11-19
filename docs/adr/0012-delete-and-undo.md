# Delete moves to Trash, and an Undo moves back by the UID Trash gave

Delete (issue #54) moves Messages to the Folder with the Trash role (RFC
6154) by `UID MOVE` (RFC 6851); Postal never expunges a Message for good. On a
server without `MOVE`, `UID COPY`, `\Deleted` and `UID EXPUNGE` stand in, the
last needing UIDPLUS (RFC 4315): a plain `EXPUNGE` would also take what
another client flagged, so without UIDPLUS the copies stay flagged.

As with flag changes (ADR 0011), a Delete shows at once and queues in the
Cache. Its row leaves `message` for the `moved` table, which keeps the whole
row, so an Undo writes it back without the server. On Gmail, where a move
to Trash drops every label, the Message's rows in other Folders go too. Trash
is looked up at send time, after a `LIST` if the Cache knows none, so a
Delete works before the Folder Picker ever opened; with no Trash, the queued
Deletes are taken back.

An Undo of a Delete still queued only drops it. One the server has made is
moved back out of Trash by the UID the server named in its `COPYUID`; without
one there is nothing to address, and the Undo says so. The row goes back
under its old UID, which the server no longer has, until the next resync of
its Folder fetches the new one and swaps it in, in the same transaction. The
Cache keeps the new UID meanwhile, so a Delete or a mark made before that
resync reaches the Message by it, and an `EXPUNGE` of the old UID, the
move's own, leaves the row alone. Writing the new UID
in directly could leave UIDs below it never fetched, as a resync asks only
for UIDs past the highest cached one. The Messages likewise reach Trash's
rows only by its own resync.

Until the server has made a move, the Cache reports the UIDs the server holds
rather than the rows it shows: a queued Delete's UID still counts and an
Undo's old one does not, or the sequence numbers a resync assigns would
misplace the `EXPUNGE`s the move brings. A resync or a page of older Messages
never writes a queued Delete's row back.

An Undo made while the server is moving that Delete marks it undone; when
the server answers, the move is queued back. The Undo stack lives for one
run, so a restart drops what only it could take back.

## Considered options

- **Delay the move until the Toast is gone, and Undo by not sending it.** No
  move back, but a Delete would reach the server only after three seconds,
  and an Undo would last only that long.
- **Flag `\Deleted` and expunge on quit.** Undo by clearing the flag, but
  Gmail by default expunges a flagged Message at once, which only drops its
  label, and a crash would leave the flags behind.
- **Rows in Trash at once, under a placeholder UID.** Trash would show the
  Message before its resync, at the cost of UIDs the server never assigned
  sitting among the ones it did.
