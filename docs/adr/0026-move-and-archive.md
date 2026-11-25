# Move and Archive go through Delete's queue, and Gmail moves by UID MOVE

Archive and Move (issue #103) take Messages out of a Folder as a Delete does
(ADR 0012): the row leaves `message` for `moved` at once, the sync
connection moves it once connected, and an Undo moves it back by the UID the
server named in its `COPYUID`. `moved` names where each Message goes: a role
(`to_role`) looked up at send time, as Trash always was, or a Folder
(`to_folder`) the Folder Picker picked. Archive is the Folder with the
`\Archive` role, else the one named `Archive`; on Gmail, which has none,
All Mail.

On Gmail a Folder is a label, and Postal moves by `UID MOVE` all the same:
Gmail makes a move from one label to another a change of the two labels,
and a move to All Mail takes off the label the Message was moved from, which
is what archiving means there. `X-GM-LABELS` would say the same in one
`STORE`, but it answers no `COPYUID`, and an Undo needs one to address the
Message where it went.

A move out of All Mail is the exception. All Mail holds every Message but
Trash's and Spam's, so expunging from it is no label change: what Gmail does
then depends on the account's IMAP settings, up to erasing it for good. So:

- A Move or an Archive from All Mail is never queued: in All Mail, `m` says
  it keeps every Message and `e` says it is archived already. A Delete from
  it still moves to Trash, as Gmail does.
- An Undo of an Archive, back out of All Mail, is a `UID COPY` alone: the
  Message keeps its place in All Mail and gets its label back.

The other Folders of a Thread show what Gmail does. A move to a label leaves
the Message's rows under its other labels, and its place in the Thread, as
they are; only a move to Trash or Spam drops every copy, as a Delete did.
The Folder moved to gets the row from a resync of it once the server made
the move, when the Cache holds it already; one never synced waits for its
own opening. Writing the row under the UID `COPYUID` names instead would make
the next resync skip mail that reached the Folder meanwhile under a lower UID.

An Undo the server has not made yet, of a Message it still has where a
first move took it, makes the Message hold still for a move to another
goal: `moved` holds one source Folder per Message, and the server has it in
neither. The move says there is nothing to move, and works once the Undo is
made.

## Considered options

- **`X-GM-LABELS` on Gmail.** One `STORE` per change and no expunge, but
  without `COPYUID` an Undo would have to search All Mail for the Message by
  `X-GM-MSGID` before it could move it back.
- **A Move or Archive queue of its own.** Delete's queue already shows the
  move at once, survives a restart and orders an Undo against the move it
  takes back; a second queue would repeat all of it.
