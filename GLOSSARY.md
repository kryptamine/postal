# Postal

A keyboard-driven terminal mail client that reads one IMAP mailbox.

## Language

**Account**:
The single mailbox identity Postal reads: an email address and its credential.
_Avoid_: user, profile, login

**Folder**:
One mailbox on the server, holding Messages; on Gmail, a label. Postal shows one Folder at a time.
_Avoid_: mailbox, label, directory

**Inbox**:
The Folder named `INBOX`, the one every run starts in.
_Avoid_: feed

**Folder Picker**:
A popup over the screen listing every Folder with its Unread count, opened with `f`; typing narrows it, `Enter` shows the chosen Folder.
_Avoid_: folder list, switcher, sidebar

**Message**:
One email as stored on the server.
_Avoid_: mail, letter, email

**Message List**:
The ordered rows of the shown Folder, one row per Thread, by its newest arrival first. A row names the senders in order of first appearance (the Account as `me`), counts the Messages when more than one, and shows the oldest subject with the newest Snippet and date.
_Avoid_: index, table, thread list

**Date Group**:
A run of Message List rows whose newest Message arrived in the same span of local time: Today, Yesterday, This Week (from Monday), Last Week, then each earlier month of this year, then each older year. Each is headed by its name, and the top one's header stays in view while scrolling. A Search has none.
_Avoid_: section, bucket, separator

**Thread**:
The Messages of one conversation, as Gmail groups them by `X-GM-THRID`; on another server, each Message is a Thread of its own. In the Message List, those in the shown Folder and the Cache, joined in its count by those found in other Folders once it has been read. Unread while any of its Messages in the shown Folder is.
_Avoid_: conversation, chain

**Unread**:
A Message the server has not marked as seen. Postal shows this state and never changes it.
_Avoid_: new, unseen

**Preview**:
The body of the newest Message of the selected Thread as Postal shows it: plain text wrapped, HTML in the rendered or the text View.
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
Entered with `o`: each distinct URL the Preview window shows, or every unfolded body on screen in Reading, gets a label of home-row letters, and typing one opens it.
_Avoid_: link mode, follow mode, footnotes

**Reading**:
The selected Thread filling the screen in place of the Message List, entered with `Enter`: its Messages in the Cache for the shown Folder at once, then on Gmail those All Mail holds in other Folders (never Trash or Spam) as the server answers, oldest first, each folded to one line (sender, Snippet, date) or unfolded to its header and body as the Preview shows it. On entry the newest and every Unread Message are unfolded, and the cursor sits on the first Unread, else the newest. `q` or `Esc` goes back to the list.
_Avoid_: reader, zoom, full view

**Search**:
The Message List narrowed to the Threads holding a Message whose sender, subject or Snippet fuzzy-match a typed Query, each ranked by its best match. It matches every Folder in the Cache but Trash and Spam, each Message once; `/` starts it, and `Esc` ends it in the shown Folder, on the selected match when that Folder holds its Thread, else where `/` was pressed (ADR 0010).
_Avoid_: filter, find, fzf, lookup

**Query**:
The text typed into a Search.
_Avoid_: pattern, search string

**Onboarding**:
The first-run flow that turns an email address into a working Account.
_Avoid_: setup wizard, registration
