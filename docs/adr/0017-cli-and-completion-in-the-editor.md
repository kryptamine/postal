# The CLI is Postal's second surface, and an editor completes recipients through it

A Draft is written in the user's `$EDITOR` (ADR 0013), where Postal cannot
reach. Expanding a recipient once the editor exits (ADR 0016) proved
surprising: nothing happens while typing, and a name typed wrong shows only
after the editor is gone. Completion belongs where the typing happens, so
Postal answers an editor's questions from the shell instead, and each
editor gets a small plugin; Neovim's is the first, in `lua/`. Without one,
a recipient is typed in full.

The same commands serve a person at a shell and an agent alike, which makes
the CLI a surface of its own beside the TUI. Its commands read
`postal <noun> <verb>`, as GitHub's `gh` does, with the nouns of
`GLOSSARY.md`; each answers in a table, or with `--json` in a JSON array
whose fields only grow. A command runs beside a TUI already open: it opens
the Cache read-only, never rebuilds it and never writes it, and when the
Cache is missing or of another schema it says so and exits with 1. It knows
only what the last sync left; `folder sync` is the one command that syncs,
and writes (ADR 0033).

`postal correspondent list` is the first: every Correspondent, ranked as
ADR 0016 ranks them, the Account a `--draft` path's notes name first, else
the one `--account` names. The plugin asks for the list once per Draft and
lets the completion engine filter it as the user types, keeping Postal's
rank as each item's score.

`postal thread show --draft <path>` prints every Message of the Thread a
Reply's Draft answers, oldest first, with its sender, recipients, date and
body as plain text, so an agent writing the Reply reads the whole
correspondence. The Draft's notes name the answered Message and its Thread;
the Thread is every Message the Cache holds with that `X-GM-THRID` in any
Folder but Trash and Spam, or among those All Mail held (ADR 0009). As the
command never writes the Cache, opening a Reply in the TUI fetches every
body of its Thread first; a body still missing is marked, not skipped. A
Draft that answers nothing, a new mail, has an empty Thread: the command
prints nothing and succeeds. `--message <path>` names a Message `E`
wrote for the editor, by the Account and the body key in its path, and
prints its Thread the same way; only the bodies already cached are there.

## Considered options

- **Expansion after the editor (ADR 0016).** Works with any editor and no
  set-up, but only once the editor is gone; removed.
- **A recipient picker before the editor opens.** A step on every Compose,
  and no help with a Cc added to a Reply.
- **The CLI asked on every keystroke.** Postal's matcher would decide, at
  the cost of a process per key; the list is a few hundred rows.
