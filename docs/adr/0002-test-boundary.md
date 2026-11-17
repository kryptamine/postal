# Tests fake the IMAP protocol, not Postal's own traits

Tests check whole behaviour through the IMAP boundary: the test itself serves
raw IMAP responses over a loopback socket in the same process (no separate
server or process), runs the real binary in a pty, and records the commands
the client sends. This keeps response parsing, a hot path, under test, and lets
a test assert the client stays read-only (`BODY.PEEK` only, never `STORE`,
`EXPUNGE` or `COPY`).

Google's OAuth server counts as part of the mail provider and is faked the same
way, at the HTTP level: the config carries undocumented `[oauth]` overrides for
the authorization and token URLs, and the browser is replaced by a command that
calls the loopback redirect.

## Considered options

- **A fake behind Postal's IMAP trait.** Faster to write, but it skips the
  protocol and parsing, cannot drive the binary in a pty, and needs a library
  entry point only tests use. Allowed only as an exception, like a unit test.
- **Unit tests.** Allowed only for a function with many invariants that is far
  cheaper to cover directly; the test says so in a comment.
