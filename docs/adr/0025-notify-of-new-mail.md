# New Inbox mail raises a desktop notification, through the terminal where it can

New Unread mail in an Inbox raises one desktop notification per batch: the
sender and subject of a Message alone, else how many came and from whom.
Mail read elsewhere already, sent by the Account itself, found by the run's
first resync of the Inbox (it came while Postal was closed), refetched under
a new UIDVALIDITY, or brought by a Backfill raises none. `notify` in
`config.toml`, and its row in Settings, says when: `unfocused` (the default,
by crossterm's focus events), `always` or `never`. Until the
terminal reports focus, Postal notifies: tmux, by default, never reports it.

## Delivery

The terminal's own `OSC 9` where it is known to show it as a desktop
notification: iTerm2, WezTerm, ghostty (`TERM_PROGRAM`) and kitty (`TERM` or
`KITTY_WINDOW_ID`), and not inside tmux or screen, which drop it unless told
to pass it on. The terminal then names itself as the sender and focuses the
right window on a click. Anywhere else, `osascript` on macOS and
`notify-send` elsewhere, started without waiting on them. A failure is only
logged.

## The Inbox while another Folder shows

IDLE watches the shown Folder alone (ADR 0008). While another Folder shows,
the sync connection ends IDLE every two minutes to send `STATUS INBOX
(UIDNEXT)`; when it differs from the Cache's, the Inbox is resynced (its
new rows reach the Cache, and the notification), then the shown Folder is
selected and resynced again.

## Considered options

- **`OSC 777`**: a title apart from the body, but iTerm2 does not show it.
- **The OS alone**: one path, but the notification names `osascript` or a
  generic sender, and a click does not bring the terminal forward.
- **A second connection idling on the Inbox**: news at once, at the cost
  ADR 0008 weighs, and Gmail's limit of 15 connections per account.
- **`STATUS` over the fetch connection**: leaves IDLE alone, but that
  connection does not know which Folder is shown, and its Inbox writes would
  race the resync of the sync connection.
