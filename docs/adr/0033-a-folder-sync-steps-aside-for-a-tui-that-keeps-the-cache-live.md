# A folder sync steps aside for a TUI that keeps the Cache live

The Cache was synced only while the TUI ran, so the CLI and the MCP server
(ADR 0017, ADR 0030) read whatever the last session left. `postal folder
sync`, the tool `folder_sync`, connects, syncs and exits. It runs only when
a person or an agent asks for it. No daemon or timer syncs on its own: open
the TUI and it syncs as before.

## What a sync does

One connection per Account, Accounts side by side, each doing what the
TUI's sync connection does on connecting:

1. It sends the flag changes and moves the Cache queues, then the Outbox.
   They are the user's own actions, queued in the TUI, so a sync sends them
   just as a TUI started again would. An agent puts nothing in the Outbox,
   so ADR 0030's rule holds: the agent never sends a Message itself.
2. It resyncs the Inbox, or the Folder `--folder` names, with its
   Snippets. A name only the TUI's Folder List knows can be given, so a
   Folder the TUI never listed cannot be named yet.
3. It fetches the bodies of the 50 newest new Messages and indexes them, so
   `message show` and `message search` see them. The TUI's passes fetch the
   rest (ADR 0032).

Each Account answers in one row: synced with its count of new Messages,
signed out, or failed with the error. A signed-out or failed Account makes
the command exit with 1, and makes the tool's result an error. An Account
gets five minutes, as no IMAP command times out on its own.

## One writer

The TUI and a sync must not run at once. Both would send the Outbox, and a
Message could go out twice, since the `.sent` rename of ADR 0013 only stops
a second send within one process's order of steps. Both would also open
the Cache to write, and `Cache::open` may rebuild it.

So whichever runs holds an exclusive lock on `sync.lock` in the Cache's
directory (`flock`, through `File::try_lock`). The OS releases it when the
process ends, however it ends.

- **A sync finding the TUI open** answers `open` for each Account, opens no
  connection, and succeeds. The TUI's IDLE keeps the Inbox live (ADR 0008),
  so the Cache is as fresh as a sync would make it.
- **A TUI started during a sync** waits for it, saying so on stderr before
  it takes the screen. A sync is bounded by its five minutes per Account.

This supersedes ADR 0017's "never writes the Cache" for this command alone.
Every other command still opens the Cache read-only.

## Considered options

- **A daemon or a timer.** It would keep mail fresh with nothing open, but
  it brings a service to install on two OSes. A sync on demand covers the
  agent's case, and a person opens the TUI.
- **A sync beside an open TUI.** It would need a lock on the Outbox and on
  the queued changes, and a way for the TUI to see rows written outside it,
  only to fetch what IDLE is already fetching.
- **A sync that fails while the TUI is open.** An agent would treat
  success as failure: the Cache is live, so there is nothing to retry.
- **`postal sync`, as issue #69 first named it.** It would break ADR 0030's
  rule that every command is a `noun verb` and every tool `noun_verb`. A
  sync acts on Folders.
