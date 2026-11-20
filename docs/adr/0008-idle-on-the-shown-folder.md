# IDLE watches only the shown Folder

Postal holds one IMAP connection, and IDLE (RFC 2177) reports changes to the
selected mailbox only. When the Folder Picker shows another Folder, IDLE moves
with it: the new Folder's arrivals and flag changes show live, and the Inbox
is not watched until the user goes back to it. Its Unread count in the Folder
Picker is as fresh as the last `STATUS` sent when the Picker opened.

With several Accounts each holds its own connection, and in All Inboxes each
idles on its own Inbox. Showing another Folder moves that Account's IDLE
alone; the others stay on the Folder they last showed.

The Folder Picker's `LIST` and `STATUS (UNSEEN)` go over the same connection:
Postal ends IDLE with `DONE`, sends them, and idles again. `STATUS` is never
sent for the selected Folder (RFC 3501 advises against it); its count comes
from `UID SEARCH UNSEEN` as in ADR 0004.

## Considered options

- **A second connection idling on the Inbox.** New mail in the Inbox would
  show from any Folder, at the cost of a second session to authenticate,
  reconnect and back off, and Gmail's limit of 15 connections per account
  shared with every other client.
- **`NOTIFY` (RFC 5465).** Watches many mailboxes on one connection, but
  Gmail does not offer it.
- **Polling the Inbox with `STATUS` on a timer.** Breaks IDLE on a schedule
  and still lags behind a push.
