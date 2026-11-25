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
A popup over the screen listing every Folder with its Unread count over its total (the total alone when nothing is Unread), and Local Drafts, opened with `f`; with more than one Account, All Inboxes first, then a section per Account headed by its address; typing narrows it, `Enter` shows the chosen Folder. Opened by a Move, it lists only the Account's other Folders, and `Enter` moves there.
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
The Messages of one conversation, as Gmail groups them by `X-GM-THRID`; on another server, by their root: the first `Message-ID` of `References`, else of `In-Reply-To`, else the Message's own. A Thread never spans Accounts. In the Message List, those in the shown Folder and the Cache, joined in its count by those found in other Folders once it has been read. Unread while any of its Messages in the shown Folder is.
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

**Pick**:
A Thread of the Message List or a Search held for the next Delete, Archive, Move or `U` to act on with the others at once: `J` and `K` move the selection down and up, picking every Thread from where the first was pressed to the selection. Each picked card is marked at its left edge, and the hint line counts them. `Esc`, any other move, another Folder, a new Search or Reading drops them, as does the action. A Move takes only one Account's Picks; `U` makes them all Read while any is Unread, else Unread as a Mark Unread does. One Undo takes back the whole action.
_Avoid_: selection (the selection is the cursor), mark, tag

**Trash**:
The Folder the server names for deleted Messages; Postal never erases a Message for good.
_Avoid_: bin, deleted items

**Delete**:
Moving Messages to Trash with `Backspace`: in the Message List or a Search, those of the selected Thread in its Folder; in Reading, the one under the cursor. It shows at once, reaches the server once connected, and does nothing in Trash.
_Avoid_: remove, erase, trash (as a verb)

**Archive**:
Moving Messages, as a Delete takes them, with `e` to the Account's Folder with the archive role (RFC 6154), or by the name `Archive` without one; on Gmail to All Mail, which takes the Inbox label off. It does nothing in that Folder.
_Avoid_: file, done

**Move**:
Moving Messages, as a Delete takes them, with `m` to the Folder the Folder Picker picks among the Account's. Not from All Mail, which holds every Message (ADR 0026).
_Avoid_: file, label (as a verb)

**Undo**:
Taking back the latest Delete, Archive, Move, Draft discard or held Send not yet taken back, with `u`, as many times as this run has made one: the Messages moved return to the Folder they left, and a discarded Draft or a held Send returns to its preview. A Send SMTP has is never taken back: its Undo is gone, and `u` takes the one before.
_Avoid_: restore, revert

**Preview**:
The body of the newest Message of the selected Thread as Postal shows it: plain text wrapped, HTML in the rendered or the text View, without its Quote. Its header names the sender, muted when it is the Account, and every `To` and `Cc` address with their names, the date at the right, and the subject. Each unfolded Message in Reading has the same header but the subject.
_Avoid_: reader, viewer, body pane

**Quote**:
The earlier mail a Message that answers another repeats at its end, with the line naming whom it is of: `>` lines, a `<blockquote>`, or all under Outlook's header (ADR 0015). The Preview and Reading leave it out and say so in one line; `z` shows it for that Message, or hides it again, and `$PAGER` and the editor `E` opens always show it. Quoted mail with words after it is no Quote.
_Avoid_: quoted text, citation, trimmed content

**Rendered View**:
How the Preview shows HTML mail by default: laid out as a mail client does, on the terminal cell grid, in the mail's colours moved onto the terminal's background. Images are never fetched.
_Avoid_: HTML mode, rich view, browser view

**Text View**:
How the Preview shows HTML mail after `v`: its text in reading order, wrapped to the pane, styled only by Postal's Theme.
_Avoid_: plain mode, raw view

**Attachment**:
A file a Message carries beside its text: a part sent as an attachment, or a non-text part with a file name; an image the HTML shows by `cid:` is none, and an attached Message counts as one. A row of the Message List whose Thread has one is marked left of its subject, and the Preview, Reading, `$PAGER` and the editor `E` opens name each with its size under the header. `a` opens one in the program the desktop picks for it, `s` saves one to the download directory; either fetches it from the server each time, as it is never cached.
_Avoid_: file, enclosure

**Invitation**:
A meeting a Message carries in a calendar part, inline or attached. Its card sits over the body in the Preview, Reading and `$PAGER`: the title, when in the local zone (and in the event's own when that differs), how it repeats, where, the link to join by, the organizer and the user's own answer. A cancelled one is marked so; an answer to the user's own invitation is one line, `Alice accepted`. Postal does not answer one: the RSVP links the sender puts in the body do, through Hint mode.
_Avoid_: event, meeting request, calendar invite

**Snippet**:
The first words of a Message's body, shown under its subject in the Message List.
_Avoid_: preview, excerpt, summary

**Cache**:
Messages kept on disk between runs, so the Message List shows before the server answers. In the background it fills with every Message of every Folder but Trash and Spam, and its search index with their bodies; the raw bodies are kept only as the Preview opens them. After a write to disk fails, it lives in memory for the rest of the session.
_Avoid_: store, database, offline copy

**Backfill**:
Fetching the next 500 Messages older than every cached one of an Account, as the selection nears the end of the Message List. Rows never leave the Cache to make room. All Inboxes shows rows only down to the latest of the Accounts' oldest cached arrivals, and a Backfill there fetches for that Account.
_Avoid_: pagination, load more, infinite scroll

**Toast**:
A short notice in the right slot of the hint line, shown for three seconds; a new one replaces the old.
_Avoid_: notification, flash, alert

**Notification**:
The desktop's notice of new Unread mail in an Inbox while Postal runs, one per batch: its sender and subject, or how many came and from whom. None for mail the first sync or a Backfill finds, read elsewhere, or sent by the Account. `notify` says when: while the terminal has no focus (the default), always or never (ADR 0025).
_Avoid_: alert, toast, popup

**Status**:
The connection state in the right slot of the hint line, silent while live: syncing (the first fetch or a Backfill), fetching what a Reply or a Forward waits on, connecting, or a countdown to the next reconnect. With more than one Account, it shows the worst state among them, named by its address. A Toast covers it while shown.
_Avoid_: indicator, banner, connection bar

**Hint mode**:
Entered with `o`: each distinct URL the Preview window shows, or every unfolded body on screen in Reading, gets a label of home-row letters, and typing one opens it. `a` and `s` label the attachments of the selected Message, in Reading of the one at the cursor, the same way.
_Avoid_: link mode, follow mode, footnotes

**Reading**:
The selected Thread filling the screen in place of the Message List, entered with `Enter`: its Messages in the Cache for the shown Folder at once, then on Gmail those All Mail holds in other Folders (never Trash or Spam) as the server answers, oldest on top and newest at the bottom, each folded to one line (sender, Snippet, date) or unfolded to its header but the Subject and its body as the Preview shows it; the border names the Thread's subject, its senders in order of first appearance and the cursor's place, and the Account's own Messages show their sender muted. Until the first fold, two or more folded in a row between the oldest and the newest take one line, `N earlier messages`. An unfolded Message scrolled past its top keeps its header pinned at the top over a rule, until the next Message pushes it up. On entry the newest and every Unread Message are unfolded, and the cursor sits on the oldest Unread, else the newest. `q` or `Esc` goes back to the list.
_Avoid_: reader, zoom, full view

**Search**:
The Message List narrowed to the Threads holding a Message that matches a typed Query: a word in its sender, subject, Snippet or body starting with each of the Query's words, not a fuzzy match, and each of its operators; each Thread ranked by its best match (ADR 0023, ADR 0031). Bodies match once the background pass has indexed them, and a Thread matched only in a body comes last; the hint line shows how far the index is, as `search index 37% (120,312 of 325,000)` (ADR 0032). A Message the Account sent matches by its recipients' names and addresses too. It matches every Folder of every Account in the Cache but Trash and Spam, unless an `in:` names one, each Message once; a Message whose flags change while it is open stays until the Query does. `/` starts it, a menu over the Query line suggests what its last word can become, `Tab` writing one in, and `Esc` closes the menu, else ends the Search in the shown Folder, on the selected match when that Folder holds its Thread, else where `/` was pressed (ADR 0010).
_Avoid_: filter, find, fzf, lookup

**Query**:
The text typed into a Search: free words, and operators in Gmail's syntax, as `from:ada`, `is:unread`, `in:Trash`, `before:2024-01-31` or `-word` (ADR 0031). An unknown operator is searched as words.
_Avoid_: pattern, search string

**Compose**:
Writing a new Message with `c` from the Account of the shown Folder; in All Inboxes, of the selected Thread; with neither, from the first Account: Postal opens `$EDITOR` on a Draft holding its headers, an empty body and the Account's Signature. An editor that ends in Send sends it; any other exit keeps it in Local Drafts, with no preview. `postal <mailto-url>` composes the same way from a `mailto:` link, its recipients, Subject and body filled in and every other header dropped; with a TUI already open, it writes the Draft beside it and opens only the editor, and the TUI lists the Draft in Local Drafts.
_Avoid_: new mail, write

**Correspondent**:
An address an Account has mail from or has written to, with the name it goes by, as the Cache knows it; those the Account wrote to rank first. On macOS, with `contacts = "macos"`, each address of a person in Contacts is one too, under the name the user gave, ranked below every address with mail. Postal never keeps an address book: Contacts are read anew each time, never copied into the Cache (ADR 0035). The CLI lists them, for an editor to complete a Draft's recipients from.
_Avoid_: contact, address book, recipient

**Reply**:
Writing a Message in answer to one with `r`, from the Account it reached, to its sender, in its Thread, with the Account's Signature above its body quoted: in the Message List the newest of the selected Thread not from the Account, in Reading the one under the cursor. **Reply All** (`R`) adds everyone else it was sent to, but the Account. Once sent, the original is flagged `\Answered`. A Reply to a Message a Draft already answers opens that Draft. While connected, its Draft opens once every body of its Thread the Message List knows is in the Cache, those missing fetched first, so `postal thread show` reads the whole correspondence.
_Avoid_: respond, answer

**Forward**:
Passing a Message on with `F`, from the Account it reached, as a new Thread: the selected Message, in Reading the one under the cursor, below a forwarded-message header naming its From, Date, Subject, To and Cc, its text as sent, an HTML one laid out as text, and the Account's Signature above it. `To` is left for the user, and the Subject gains `Fwd: ` once. Its attachments are fetched first and attached on `Attach:` lines (ADR 0027).
_Avoid_: resend

**Signature**:
The lines an Account ends its Messages with, under a `-- ` line (RFC 3676): `signature` in `config.toml`, or the text file `signature-file` names, `~` the home directory and a relative path beside `config.toml`; at the top level for the Account named there, in an `[[account]]` for that one. Compose, Reply and Forward write it into the Draft, where it is the user's to change; `Enter` on its row under the Account in Settings opens `$EDITOR` on a copy of it, which the plugin's agent can write as it writes a Draft, and takes what the copy holds once the editor exits. In the Preview and Reading, the Signature of a plain-text Message shows muted, down to its Quote; an HTML one shows as the mail lays it out.
_Avoid_: footer, sign-off

**Draft**:
A Message being written, kept on this machine until it is sent or discarded (ADR 0013); the editor's exit after Compose, Reply or Forward, or `d` in its preview, keeps it for later. Its file holds `To`, `Cc`, `Subject` and `Attach` lines, one file attached per `Attach` line, then the body; the files are read once, on Send, into the Outbox. With `compose-format = "markdown"` in `config.toml`, at the top level or in an `[[account]]`, the body is Markdown: Send adds its HTML beside the text as written, the signature, a Quote and a Forward's original kept as written, and the preview shows that HTML in the Rendered View, `v` the text (ADR 0036).
_Avoid_: unsent mail, composition

**Local Drafts**:
Postal's own Folder in the Folder Picker, never on the server: every Draft and every Outbox Message, newest first, with its recipients and subject, and its sender when there is more than one Account. A held Send shows how long it has left. `Enter` opens a Draft's preview, or shows an Outbox Message read-only with its last error; `Backspace` discards a Draft.
_Avoid_: Drafts (the server's Folder), unsent

**Send**:
Handing a Draft to the server over SMTP, from its preview with `y` or from the editor with the plugin's `:PostalSend` (ADR 0037); one that does not parse opens its preview with the reason. It waits in the Outbox until the server accepts it. It is first held there for `send-delay` seconds in `config.toml`, 10 unless set and none for `0`, while a Toast counts down and `u` takes it back to the Drafts; quit inside that time, it leaves on the next sync.
_Avoid_: submit, deliver

**Outbox**:
The Messages sent but not yet accepted by the server, retried on each sync (ADR 0013).
_Avoid_: queue, pending, spool

**Onboarding**:
The screen that signs an Account in: on a first run, and after `postal login` or Add account in Settings to add one, from the Provider list, then in the browser for Google, by an app-specific password for iCloud, or by the servers, as the domain publishes them, and a password for any other, checked on the server before anything is written. Signing in an address again replaces its secret. `L` and a lost secret skip the Provider list; for a lost secret it shows before the run, once per Account, and `Esc` runs without that Account. A failure is shown there and can be tried again.
_Avoid_: setup wizard, registration

**Settings**:
A popup over the screen, opened with `,`, that changes `config.toml` without editing it, each change written at once. A tab per section, switched with `Tab` and `S-Tab`, says what it sets: Accounts, each with its Provider and under it its Signature by its first line, then Add account; Icons, Notify, Mailto and Secrets, each a list of values, the one in effect ticked, `Enter` applying the one selected; Name; and Send, the format of a Draft's body, a list of values, and the seconds a Send is held. Mailto also switches whether the system hands `mailto:` links to Postal, and on macOS types a program of the user's own to open them in. `Enter` on a Signature opens `$EDITOR` on it; `d` unlinks an Account after `y`: it leaves the config, its secret and its Cache rows go. A change that the Accounts read at start, the Name, the format, the Send delay, a Signature or the secret store, opens the TUI again when Settings closes.
_Avoid_: preferences, options, config screen

**Unlink**:
Taking an Account out of Postal from Settings: out of `config.toml`, its secret out of its store, its rows out of the Cache. Its Drafts and Outbox Messages stay.
_Avoid_: remove, sign out, log out

**Provider**:
The mail service an Account lives at, which decides how it signs in. Google, signed in through the browser; iCloud, by an app-specific password; or Other, any IMAP/SMTP server, by the servers typed and a password.
_Avoid_: service, host, backend

**All Inboxes**:
The Inbox of every Account in one Message List, by newest arrival, at the top of the Folder Picker and where every run starts when there is more than one Account. Every other Folder belongs to one Account.
_Avoid_: unified inbox, combined inbox, merged view

**CLI**:
Postal's commands run from a shell or by an agent, beside the TUI or without it: `postal <noun> <verb>`, the nouns those of this glossary, each answering in a table, or in JSON with `--json`.
_Avoid_: API, scripting mode
