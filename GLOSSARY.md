# Postal

A keyboard-driven terminal mail client that reads one or more IMAP mailboxes.

## Language

**Account**:
One mailbox identity Postal reads: an email address at a Provider and its credential. Postal may hold several.
_Avoid_: user, profile, login

**Folder**:
One mailbox on the server, holding Messages; on Gmail, a label. Postal shows one Folder at a time.
_Avoid_: mailbox, label, directory

**Inbox**:
The Folder named `INBOX` of an Account; with one Account, the one every run starts in.
_Avoid_: feed

**Folder Picker**:
A popup over the screen listing every Folder with its Unread count, and Local Drafts, opened with `f`; with more than one Account, All Inboxes first, then a section per Account headed by its address; typing narrows it, `Enter` shows the chosen Folder.
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
The Messages of one conversation, as Gmail groups them by `X-GM-THRID`; on another server, each Message is a Thread of its own. A Thread never spans Accounts. In the Message List, those in the shown Folder and the Cache, joined in its count by those found in other Folders once it has been read. Unread while any of its Messages in the shown Folder is.
_Avoid_: conversation, chain

**Unread**:
A Message the server has not marked as seen. Postal clears it by a Mark Read.
_Avoid_: new, unseen

**Dwell**:
The selection resting on one Thread for a second after its Preview shows the body; moving on first cancels it.
_Avoid_: hover, linger, debounce

**Mark Read**:
Clearing Unread on every Unread Message of a Thread in its Folder: after a Dwell in the Message List or a Search, at once on entering Reading. It shows at once and reaches the server once connected.
_Avoid_: mark seen, read receipt

**Mark Unread**:
Making a Message Unread again with `U`: the newest of the selected Thread, or the one under the cursor in Reading. It shows at once, and no Dwell starts on that Thread until the selection leaves it and comes back.
_Avoid_: unmark, keep unread

**Trash**:
The Folder the server names for deleted Messages; Postal never erases a Message for good.
_Avoid_: bin, deleted items

**Delete**:
Moving Messages to Trash with `Backspace`: in the Message List or a Search, those of the selected Thread in its Folder; in Reading, the one under the cursor. It shows at once, reaches the server once connected, and does nothing in Trash.
_Avoid_: remove, erase, trash (as a verb)

**Undo**:
Taking back the latest Delete or Draft discard not yet taken back, with `u`, as many times as this run has made one: a Delete's Messages return to the Folder they left, and a discarded Draft returns to its preview.
_Avoid_: restore, revert

**Preview**:
The body of the newest Message of the selected Thread as Postal shows it: plain text wrapped, HTML in the rendered or the text View, without its Quote. Its header names the sender and every `To` and `Cc` address with their names, the date at the right, and the subject. Each unfolded Message in Reading has the same header.
_Avoid_: reader, viewer, body pane

**Quote**:
The earlier mail a Message that answers another repeats at its end, with the line naming whom it is of: `>` lines, a `<blockquote>`, or all under Outlook's header (ADR 0015). The Preview and Reading leave it out and say so in one line; `z` shows it for that Message, or hides it again, and `$PAGER` always shows it. Quoted mail with words after it is no Quote.
_Avoid_: quoted text, citation, trimmed content

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
Fetching the next 500 Messages older than every cached one of an Account, as the selection nears the end of the Message List. Rows never leave the Cache to make room. All Inboxes shows rows only down to the latest of the Accounts' oldest cached arrivals, and a Backfill there fetches for that Account.
_Avoid_: pagination, load more, infinite scroll

**Toast**:
A short notice in the right slot of the hint line, shown for three seconds; a new one replaces the old.
_Avoid_: notification, flash, alert

**Status**:
The connection state in the right slot of the hint line, silent while live: syncing (the first fetch or a Backfill), fetching the body a Reply waits on, connecting, or a countdown to the next reconnect. With more than one Account, it shows the worst state among them, named by its address. A Toast covers it while shown.
_Avoid_: indicator, banner, connection bar

**Hint mode**:
Entered with `o`: each distinct URL the Preview window shows, or every unfolded body on screen in Reading, gets a label of home-row letters, and typing one opens it.
_Avoid_: link mode, follow mode, footnotes

**Reading**:
The selected Thread filling the screen in place of the Message List, entered with `Enter`: its Messages in the Cache for the shown Folder at once, then on Gmail those All Mail holds in other Folders (never Trash or Spam) as the server answers, the oldest on top and the rest under it newest first, each folded to one line (sender, Snippet, date) or unfolded to its header and body as the Preview shows it. An unfolded Message scrolled past its top keeps its header pinned at the top over a rule, until the next Message pushes it up. On entry the newest and every Unread Message are unfolded, and the cursor sits on the oldest Unread, else the newest. `q` or `Esc` goes back to the list.
_Avoid_: reader, zoom, full view

**Search**:
The Message List narrowed to the Threads holding a Message whose sender, subject or Snippet fuzzy-match a typed Query, each ranked by its best match. It matches every Folder of every Account in the Cache but Trash and Spam, each Message once; `/` starts it, and `Esc` ends it in the shown Folder, on the selected match when that Folder holds its Thread, else where `/` was pressed (ADR 0010).
_Avoid_: filter, find, fzf, lookup

**Query**:
The text typed into a Search.
_Avoid_: pattern, search string

**Compose**:
Writing a new Message with `c` from the Account of the shown Folder; in All Inboxes, of the selected Thread; with neither, from the first Account: Postal opens `$EDITOR` on a Draft holding its headers and an empty body.
_Avoid_: new mail, write

**Correspondent**:
An address an Account has mail from or has written to, with the name it goes by, as the Cache knows it; those the Account wrote to rank first. Postal learns them from mail alone and never keeps an address book. The CLI lists them, for an editor to complete a Draft's recipients from.
_Avoid_: contact, address book, recipient

**Reply**:
Writing a Message in answer to one with `r`, from the Account it reached, to its sender, in its Thread, with its body quoted: in the Message List the newest of the selected Thread not from the Account, in Reading the one under the cursor. **Reply All** (`R`) adds everyone else it was sent to, but the Account. Once sent, the original is flagged `\Answered`. A Reply to a Message a Draft already answers opens that Draft. While connected, its Draft opens once every body of its Thread the Message List knows is in the Cache, those missing fetched first, so `postal thread show` reads the whole correspondence.
_Avoid_: respond, answer

**Draft**:
A Message being written, kept on this machine until it is sent or discarded (ADR 0013); `d` in its preview keeps it for later.
_Avoid_: unsent mail, composition

**Local Drafts**:
Postal's own Folder in the Folder Picker, never on the server: every Draft and every Outbox Message, newest first, with its recipients and subject, and its sender when there is more than one Account. `Enter` opens a Draft's preview, or shows an Outbox Message read-only with its last error; `Backspace` discards a Draft.
_Avoid_: Drafts (the server's Folder), unsent

**Send**:
Handing a Draft to the server over SMTP from its preview with `y`; it waits in the Outbox until the server accepts it.
_Avoid_: submit, deliver

**Outbox**:
The Messages sent but not yet accepted by the server, retried on each sync (ADR 0013).
_Avoid_: queue, pending, spool

**Onboarding**:
The screen that signs an Account in: on a first run, and after `postal login` or Add account in Settings to add one, from the Provider list, then in the browser for Google or by an app-specific password for iCloud, checked on the server before anything is written. Signing in an address again replaces its secret. `L` and a lost secret skip the Provider list; for a lost secret it shows before the run, once per Account, and `Esc` runs without that Account. A failure is shown there and can be tried again.
_Avoid_: setup wizard, registration

**Settings**:
A popup over the screen, opened with `,`, that changes `config.toml` without editing it, each change written at once: the Accounts, each with its Provider, then Add account, and the General settings, Icons, Name and Secrets. `d` unlinks an Account after `y`: it leaves the config, its secret and its Cache rows go. A change that the Accounts read at start, the Name or the secret store, opens the TUI again when Settings closes.
_Avoid_: preferences, options, config screen

**Unlink**:
Taking an Account out of Postal from Settings: out of `config.toml`, its secret out of its store, its rows out of the Cache. Its Drafts and Outbox Messages stay.
_Avoid_: remove, sign out, log out

**Provider**:
The mail service an Account lives at, which decides how it signs in. Google, signed in through the browser, or iCloud, by an app-specific password.
_Avoid_: service, host, backend

**All Inboxes**:
The Inbox of every Account in one Message List, by newest arrival, at the top of the Folder Picker and where every run starts when there is more than one Account. Every other Folder belongs to one Account.
_Avoid_: unified inbox, combined inbox, merged view

**CLI**:
Postal's commands run from a shell or by an agent, beside the TUI or without it: `postal <noun> <verb>`, the nouns those of this glossary, each answering in a table, or in JSON with `--json`.
_Avoid_: API, scripting mode
