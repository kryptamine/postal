# The shown Folder is selected, and flag changes queue in the Cache

Mark Read and Mark Unread (issue #53) write `\Seen` with `UID STORE`, which
a mailbox opened by `EXAMINE` refuses. The shown Folder is now opened with
`SELECT`, so the one connection that idles on it (ADR 0008) can also write
to it. Bodies are still fetched with `BODY.PEEK`, so reading a body never
marks it seen; only a Mark Read does.

A flag change shows at once, and is written to the Cache as pending before
it is sent. It leaves the pending set only once the server acknowledges the
`STORE`. Pending changes outlive a reconnect and a restart, and are sent once
the connection is back. A resync applies the server's flags to every Message
except those with a pending change, so an older server state never undoes a
mark the server has not seen yet.

A change to a Message in another Folder, as from a Search match, is sent by
selecting that Folder, storing, and selecting the shown Folder again, as
Reading already does for All Mail.

## Considered options

- **`EXAMINE`, with a brief `SELECT` around each `STORE`.** Keeps the session
  read-only by default, at the cost of breaking IDLE and a round trip for
  every mark in the shown Folder, the common case.
- **Sending without a queue, reverting on failure with a Toast.** Simpler,
  but a reconnect is routine, and marks made offline would be lost.
- **A second connection for writes.** Leaves IDLE undisturbed, at the cost of
  a second session to authenticate and back off, and Gmail's connection limit.
