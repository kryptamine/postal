# Postal

A keyboard-driven terminal mail client that reads one IMAP mailbox.

## Language

**Account**:
The single mailbox identity Postal reads: an email address and its credential.
_Avoid_: user, profile, login

**Inbox**:
The server's `INBOX` folder, the only folder Postal shows.
_Avoid_: mailbox, feed

**Message**:
One email as stored on the server.
_Avoid_: mail, letter, email

**Message List**:
The ordered rows of Messages in the Inbox, one row per Message, newest arrival first.
_Avoid_: index, table, thread list

**Unread**:
A Message the server has not marked as seen. Postal shows this state and never changes it.
_Avoid_: new, unseen

**Preview**:
The body of the selected Message as Postal shows it: plain text wrapped, HTML in the rendered or the text View.
_Avoid_: reader, viewer, body pane

**Rendered View**:
How the Preview shows HTML mail by default: laid out as a mail client does, on the terminal cell grid, in the mail's colours moved onto the terminal's background. Images are never fetched.
_Avoid_: HTML mode, rich view, browser view

**Text View**:
How the Preview shows HTML mail after `v`: its text in reading order, wrapped to the pane, styled only by Postal's Theme.
_Avoid_: plain mode, raw view

**Snippet**:
The first words of a Message's body, shown under its subject in the Message List.
_Avoid_: preview, excerpt, summary

**Cache**:
Messages kept on disk between runs, so the Message List shows before the server answers. After a write to disk fails, it lives in memory for the rest of the session.
_Avoid_: store, database, offline copy

**Backfill**:
Fetching the next 500 Messages older than every cached one, as the selection nears the end of the Message List. Rows never leave the Cache to make room.
_Avoid_: pagination, load more, infinite scroll

**Toast**:
A short notice in the right slot of the hint line, shown for three seconds; a new one replaces the old.
_Avoid_: notification, flash, alert

**Status**:
The connection state in the right slot of the hint line, silent while live: syncing (the first fetch or a Backfill), connecting, or a countdown to the next reconnect. A Toast covers it while shown.
_Avoid_: indicator, banner, connection bar

**Hint mode**:
Entered with `o`: each distinct URL the Preview window shows gets a label of home-row letters, and typing one opens it.
_Avoid_: link mode, follow mode, footnotes

**Reading**:
The selected Message's Preview filling the screen in place of the Message List, entered with `Enter`; `q` or `Esc` goes back to the list.
_Avoid_: reader, zoom, full view

**Search**:
The Message List narrowed to the Messages whose sender, subject or Snippet fuzzy-match a typed Query, best match first; `/` starts it, `Esc` ends it.
_Avoid_: filter, find, fzf, lookup

**Query**:
The text typed into a Search.
_Avoid_: pattern, search string

**Onboarding**:
The first-run flow that turns an email address into a working Account.
_Avoid_: setup wizard, registration
