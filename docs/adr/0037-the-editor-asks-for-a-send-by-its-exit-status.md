# The editor asks for a Send by its exit status

After Compose, Reply or Forward the TUI showed the Draft's preview, and the
user pressed `y` there to send. They had just read the mail in the editor,
so the preview showed it a second time (issue #218). Now the editor itself
can end in Send, and an editor that only exits keeps the Draft in Local
Drafts with a Toast, `Saved to Drafts`. `e` from a preview still returns to
that preview: it is where the user came from, and how an editor without the
plugin sends, from Local Drafts.

The TUI waits for the editor's process, so the editor answers in the one
thing that process leaves: its exit status. Postal runs the editor on a
Draft with `$POSTAL_SEND_STATUS` set to the status that sends, `100`, not
`1`, which an editor's own error and a bare `:cquit` give. The plugin's
`:PostalSend` checks the Draft, writes it and runs `:cquit 100`. It exists
only where the variable is set, so a Draft opened beside a running TUI
(ADR 0033), whose editor no TUI waits on, offers no Send it could not keep.
After that status the TUI sends as `y` does, with the same hold and `u`.

## Considered options

- **A mark in the Draft's notes.** It outlives the editor: a TUI that died
  before reading it would send the Draft the next time it is opened.
- **`postal draft send` from the editor.** The CLI never writes the Cache
  and holds no Send for `u` (ADR 0017); the TUI already waits beside it.
- **Keep the preview after every editor.** The step this ADR removes.
