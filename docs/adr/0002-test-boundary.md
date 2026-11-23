# Tests fake the IMAP protocol, not Postal's own traits

Tests check whole behaviour through the IMAP boundary: the test itself serves
raw IMAP responses over a loopback socket in the same process (no separate
server or process), runs the real binary in a pty, and records the commands
the client sends. This keeps response parsing, a hot path, under test, and lets
a test assert what the client may change: `BODY.PEEK` only, `STORE` of
`\Seen` and `\Answered`, and `MOVE` only to Trash and back, or the `COPY`,
`\Deleted` and `UID EXPUNGE` standing in for it (ADR 0012).

Google's OAuth server counts as part of the mail provider and is faked the same
way, at the HTTP level: the config carries undocumented `[oauth]` overrides for
the authorization and token URLs and the redirect timeout. The browser is the
recording `open` stand-in ahead on `PATH`; the test reads the consent URL it was
given and calls the loopback redirect itself, as Google would.

SMTP is faked the same way, in plaintext on loopback: the test server checks
the sign-in, records each Message as it arrives after `DATA`, and can refuse
it with a reply the test picks. It answers `STARTTLS` and stops there, as no
certificate the client trusts is at hand: a test checks only that the client
asks for it before it signs in. `$EDITOR` is a command that copies the text a
test wrote over the Draft, as if the user had typed it.

Where a domain publishes its servers is faked the same way: the domain's
autoconfig and Thunderbird's ISPDB over plain HTTP, and a DNS server answering
SRV queries over UDP, both on loopback. The undocumented `[autoconfig]` table
points Postal at them; a test that needs no lookup points it at ones that
publish nothing, so no test reaches the network.

GitHub's releases page is faked the same way, for `postal update`: the
undocumented `POSTAL_RELEASES_URL` points Postal at a server whose `latest`
redirects to a tag and which serves that tag's archive and sha256.

Login reads an existing `config.toml` and keeps its `name`, `[imap]`, `[smtp]`
and `[secrets]`, so a test seeds a config without `email` that points at the
fake server and forces the file store; the login fills in the Account.

The OS keychain is not faked. Tests force `[secrets] store = "file"`, and the
undocumented `POSTAL_KEYCHAIN_FAIL` variable makes the keychain refuse access,
with its value as the reason, to cover the error path. Reading a real keychain
entry is checked by hand.

## Considered options

- **A fake behind Postal's IMAP trait.** Faster to write, but it skips the
  protocol and parsing, cannot drive the binary in a pty, and needs a library
  entry point only tests use. Allowed only as an exception, like a unit test.
- **Unit tests.** Allowed only for a function with many invariants that is far
  cheaper to cover directly; the test says so in a comment.
