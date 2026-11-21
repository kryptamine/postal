# Architecture

`src/` holds one directory per layer. `main.rs` alone sits at the root: it
declares the layers and wires one run. A layer's `mod.rs` only declares its
modules, and callers name them in full, `crate::<layer>::<module>`. `cache`
is the one layer that is a single module: its `mod.rs` re-exports `Cache`
and the few items callers use.

## Codemap

Lowest first:

- `domain/`: what mail is: Message, Folder, Thread, flags, UIDs and the other
  ids, the parts a body is cut into, one value per Account.
- `cache/`: the `SQLite` store of Messages, bodies, flags and queued moves.
- `account/`: `config.toml`, the Providers, OAuth, secrets and the keychain, the
  XDG directories, unlinking an Account.
- `render/`: an HTML part as Preview lines (html2text) or as the rendered View
  (litehtml in its layout process, the cell grid), Quotes, wrapping, the
  glyphs the font has, how a date reads.
- `compose/`: Drafts and their notes, Replies, the Outbox on disk, the editor,
  the Account a Message goes out from.
- `mail/`: IMAP and SMTP, the sync, IDLE, the fetches, the Outbox flush,
  reconnecting, the sign-in check, and what the connections tell the screen
  (`Update`, `Status`, `Steering`).
- `ui/`: the TUI: the event loop, panes, popups, the Theme, the
  keymap, the Onboarding and Settings, the log file.
- `cli/`: the commands beside the TUI: the arguments, `correspondent list`,
  `thread show`.

## Direction

A layer imports only from the layers listed before it. `ui` and `cli` are
peers: neither imports the other. `main.rs` may import any layer.

Where a module could sit in two layers, its imports decided:

- `mail` sits over `compose`: the Outbox flush sends what `compose` wrote.
- `mail` sits over `render`: an `Update` carries the Previews the fetch work
  rendered.
- `mail::login` checks a sign-in by connecting and examining the Inbox, so it
  is `mail`, though it writes `config.toml` after.
- `cli::correspondent` ranks Correspondents for `correspondent list` alone.
- `compose::accounts` and `compose::outgoing` are the Accounts as senders:
  `Outgoing` builds a Draft's envelope, so they sit over `compose::draft`.
- `render::stack_str` formats frame text without allocating; `mail::status`
  writes its text with it, so it sits under `mail`.
- `ui::logging` and `ui::panic_hook` are the TUI's: a command beside it leaves
  the log alone, and the hook restores the terminal.

## Invariants

- `domain` does no I/O and imports nothing else of the crate. Its ids carry
  rusqlite's `ToSql` and `FromSql`, never a query.
- `cache` never touches the network.
- `account`, `render` and `compose` never speak IMAP or SMTP; `account`'s only
  network is OAuth's HTTP. `compose` builds
  addresses and envelopes with lettre; only `mail` sends.
- `render` reads no files and opens no sockets; its one I/O is the layout
  process `render::render_process` runs.
- `ui` never speaks IMAP: it imports neither `mail::imap` nor `mail::smtp`,
  and asks the connections through `Steering` and the fetch requests.
- Nothing outside `ui` draws: ratatui and crossterm are imported in `ui` alone.
