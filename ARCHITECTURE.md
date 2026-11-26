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
  XDG directories, an Account's signature, unlinking an Account, the people
  in macOS Contacts (`account::contacts`, read in a process of its own by
  `account::contacts_process`), the
  system's mailto: handler that hands links to Postal (`account::mailto_handler`).
- `render/`: an HTML part as Preview lines (html2text) or as the rendered View
  (litehtml in its layout process, the cell grid), Quotes, signatures, wrapping,
  the glyphs the font has, how a date reads, the card of an Invitation.
- `compose/`: Drafts and their notes, Replies, Forwards and the files they
  attach, the `mailto:` link a Draft is filled from, the Outbox on disk, the
  editor, the Account a Message goes out from, its signature in the editor.
- `mail/`: IMAP and SMTP, the sync, IDLE, the fetches, the Outbox flush,
  reconnecting, the sign-in check, the servers a domain publishes
  (autoconfig, DNS SRV), one Account's two connections
  (`mail::serve`), one sync on demand (`mail::folder_sync`) and the lock
  it shares with the TUI (`mail::sync_lock`), what a takeover from
  another terminal waits on and sends (`mail::held`, `mail::handover`),
  and what they tell the screen (`Update`, `Status`, `Steering`).
- `ui/`: the TUI. At its root the event loop (`run`) and what it hands
  each reaction (`World`); then `state/` (the Screen, what the panes show,
  the Mode, Toasts, Undo), `input/` (the keymap, a key's `Step`), `react/`
  (what a `Step`, a `Steer`, an `Update` or a timer does), `frame/` (the
  draw, the layout, the Theme), `terminal/` (the backend, input, the tty),
  `list/` (the Message List, Search, marking, and moving Messages out
  of a Folder), `preview/` (the
  Preview, the Thread, link hints), `drafts/` (composing, the Draft and
  Local Drafts panes), `popup/` (the layers over the screen: help, the
  Folder Picker, Settings), `takeover/` (asking a TUI in another terminal
  to move here, and leaving for one), `widgets/` (small drawing pieces), `outside/`
  (the pager, opening a Message elsewhere, attachments opened or saved, the
  clipboard), `onboarding/`
  and `logging/`.
- `cli/`: the commands beside the TUI: the arguments, `correspondent list`,
  `folder list` and `sync`, `message list`, `show` and `search`, `thread
  show`, the IDs they hand out, `mcp`, which serves each command as an MCP tool,
  `update` and the releases it reads from GitHub, `setup-mailto`.
- `app/`: the TUI's run and `postal login`, the thread that hears a
  takeover's SIGTERM (`app::moved`), and `postal <mailto-url>`
  beside a TUI already open or in the terminal `config.toml` chooses
  (`app::in_terminal`, `app::link_script`): the Accounts signed in, the
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
- `cli::mcp` is a surface of its own, but each tool is a command's answer,
  so it sits with them.
- `compose::accounts` and `compose::outgoing` are the Accounts as senders:
  `Outgoing` builds a Draft's envelope, so they sit over `compose::draft`.
- `render::stack_str` formats frame text without allocating; `mail::status`
  writes its text with it, so it sits under `mail`.
- `ui::logging` and `ui::panic_hook` are the TUI's: a command beside it leaves
  the log alone, and the hook restores the terminal.

## Invariants

- `domain` does no I/O and imports nothing else of the crate. Its ids carry
  no `ToSql` or `FromSql`: the Cache maps them, and names `rusqlite` alone.
- `cache` never touches the network.
- `account`, `render` and `compose` never speak IMAP or SMTP; `account`'s only
  network is OAuth's HTTP. Looking up servers is `mail::discover`'s. `compose` builds
  addresses and envelopes with lettre; only `mail` sends.
- `render` reads no files and opens no sockets; its one I/O is the layout
  process `render::render_process` runs.
- `ui` never speaks IMAP: `mail::imap` and `mail::smtp` are private to
  `mail`, and the screen asks the connections through `Steering` and the
  fetch requests.
- Nothing outside `ui` draws: ratatui and crossterm are imported in `ui` alone.
