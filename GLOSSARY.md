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
The ordered rows of Messages in the Inbox, newest first.
_Avoid_: index, table, thread list

**Preview**:
The rendered body of the selected Message, cleaned of markup.
_Avoid_: reader, viewer, body pane

**Cache**:
Messages kept on disk between runs, so the Message List shows before the server answers.
_Avoid_: store, database, offline copy

**Onboarding**:
The first-run flow that turns an email address into a working Account.
_Avoid_: setup wizard, registration
