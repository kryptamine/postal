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

Each Message in `moved` holds two facts: whether the user wants it in Trash,
and where the server has it, in Trash or in its Folder, under which UID. A
Delete or an Undo only sets the first; the sync connection moves every
Message where the two disagree, by the UID the server last named, and writes
the answer into the second. So an Undo made while its Delete is under way,
or a Delete made while its Undo is, needs no case of its own: once the
server answers, the two disagree again and the next pass moves the Message
back. An Undo of a Delete the server has not seen leaves them agreeing, and
nothing is sent.

A move out of Trash needs the UID the server named in its `COPYUID`; without
one there is nothing to address, and the Undo says so. The row goes back
under its old UID, which the server no longer has, until the next resync of
its Folder fetches the new one and swaps it in, in the same transaction.
Writing the new UID in directly could leave UIDs below it never fetched, as
a resync asks only for UIDs past the highest cached one. The Messages
likewise reach Trash's rows only by its own resync.

Two views name what the rest of the Cache must know. `deleted` lists what
the server still holds in a Folder that the Message List leaves out: a
resync counts those UIDs when it numbers the Folder, so the `EXPUNGE`s a move
brings land on the right rows, and never writes those rows back. `undone`
lists the rows an Undo put back: an `EXPUNGE` of one is its own move's and
leaves it, and a mark on one goes by the UID the server has it under. The
Undo stack lives for one run, so a restart drops what only it could take
back.

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
