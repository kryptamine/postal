# Architecture

`src/` holds one directory per layer. `main.rs` alone sits at the root: it
declares the layers, reads the arguments and hands the command to `cli` or
`app`. A layer's `mod.rs` only declares its modules, and callers name them
in full, `crate::<layer>::<module>`. A module only its own layer names is
private: `mod`, not `pub mod`, so the compiler keeps the others out of it.
`cache` is the one layer that is a single module: its `mod.rs` re-exports
`Cache` and the few items callers use.

`ui` is large enough to group its modules by feature, one directory each:
`crate::ui::<group>::<module>`. A group's module another group names is
`pub mod`; the few other layers name are re-exported from `ui/mod.rs`, so
their paths stay `crate::ui::<module>`.

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
  reconnecting, the sign-in check, one Account's two connections
  (`mail::serve`), and what they tell the screen (`Update`, `Status`,
  `Steering`).
- `ui/`: the TUI. At its root the event loop, the Screen, the keymap, the
  layout and the Theme; then `terminal/` (the backend, input, the tty),
  `list/` (the Message List, Search, marking and deleting), `preview/` (the
  Preview, the Thread, link hints), `drafts/` (composing, the Draft and
  Local Drafts panes), `popup/` (the layers over the screen: help, the
  Folder Picker, Settings), `widgets/` (small drawing pieces), `outside/`
  (the pager, opening a Message elsewhere, the clipboard), `onboarding/`
  and `logging/`.
- `cli/`: the commands beside the TUI: the arguments, `correspondent list`,
  `thread show`.
- `app/`: the TUI's run and `postal login`: the Accounts signed in, the
  Onboarding when one is not, each Account's connections and the screen
  over them, opened again after Settings or a sign-in.

## Direction

A layer imports only from the layers listed before it. `ui` and `cli` are
peers: neither imports the other. `app` may import any layer but `cli`, and
`main.rs` any layer.

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
- `ui` never speaks IMAP: `mail::imap` and `mail::smtp` are private to
  `mail`, and the screen asks the connections through `Steering` and the
  fetch requests.
- Nothing outside `ui` draws: ratatui and crossterm are imported in `ui` alone.
