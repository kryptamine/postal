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
The rendered body of the selected Message, cleaned of markup.
_Avoid_: reader, viewer, body pane

**Snippet**:
The first words of a Message's body, shown under its subject in the Message List.
_Avoid_: preview, excerpt, summary

**Cache**:
Messages kept on disk between runs, so the Message List shows before the server answers.
_Avoid_: store, database, offline copy

**Toast**:
A short notice in the right slot of the hint line, shown for three seconds; a new one replaces the old.
_Avoid_: notification, flash, alert

**Onboarding**:
The first-run flow that turns an email address into a working Account.
_Avoid_: setup wizard, registration
