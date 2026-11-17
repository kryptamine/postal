# Research: integration testing against a fake IMAP server

Issue: [#7](https://github.com/kryptamine/postal/issues/7), part of map [#1](https://github.com/kryptamine/postal/issues/1).
Researched: 2026-11-17.

## Question

How do we test Postal end-to-end through its external boundary? The
`AGENTS.md` test rule allows only integration tests through an external
boundary. Only external processes (mail servers) may be faked. This note covers:

1. A scriptable fake IMAP server: in-process or spawned, an existing crate, or a
   real server (GreenMail, Dovecot) in a container.
2. Simulating `IDLE` notifications, `XOAUTH2`, and the Gmail extensions.
3. Driving and asserting the TUI: ratatui `TestBackend` or a pty harness.

## What Postal needs from a fake

The scope in #1 sets the requirements:

- `INBOX` only, read-only. The client fetches with `BODY.PEEK` and never sets flags.
- Gmail first, so the fake must advertise `X-GM-EXT-1` and answer the `X-GM-*`
  fetch items if Postal uses them.
- New mail arrives through `IDLE`, with a reconnect when the connection drops.
  Tests have to push `EXISTS` on demand and cut the connection mid-session.
- Browser OAuth for Gmail means `AUTHENTICATE XOAUTH2`.
- Error UX is still open (#1): expired auth, network drop, server refusal.
  Tests need to inject `NO`/`BAD` responses and malformed data.

## Findings

### Client side: the IMAP crate does not force a real socket

- `async-imap`'s `Client<T>` is generic over the stream: `T: Read + Write +
  Unpin + Debug` (futures-io `AsyncRead`/`AsyncWrite`), plus `Send` on
  `Client::new`. Any TCP stream to a local fake works.
  [docs.rs: async_imap::Client](https://docs.rs/async-imap/latest/async_imap/struct.Client.html)
- `Client::authenticate(auth_type, authenticator)` takes a pluggable
  `Authenticator` trait, so `XOAUTH2` is a client-side string builder.
  [docs.rs: async_imap](https://docs.rs/async-imap/latest/async_imap/)
- IDLE support lives in `async_imap::extensions::idle` (`Handle`, `IdleStream`,
  `IdleResponse`).
  [docs.rs: async_imap::extensions::idle](https://docs.rs/async-imap/latest/async_imap/extensions/idle/index.html)

Client crate choice is a separate ticket. Here it only matters that the client
accepts a plain TCP stream, so the fake can sit behind a socket.

### Gmail protocol facts the fake must reproduce

- XOAUTH2 is a SASL mechanism advertised as `AUTH=XOAUTH2`. The client sends
  `AUTHENTICATE XOAUTH2` with a base64 string of
  `user=<email>^Aauth=Bearer <token>^A^A`.
  [Google: OAuth 2.0 Mechanism](https://developers.google.com/workspace/gmail/imap/xoauth2-protocol)
- The Gmail extensions are signalled by the `X-GM-EXT-1` capability and add
  `X-GM-MSGID`, `X-GM-THRID`, `X-GM-LABELS` and `X-GM-RAW`.
  [Google: IMAP Extensions](https://developers.google.com/workspace/gmail/imap/imap-extensions)

### Option A: GreenMail (spawned Java process or Docker container)

- Supports SMTP, POP3 and IMAP with TLS. The Docker image is
  `greenmail/standalone`, with IMAP on 3143 and IMAPS on 3993.
  [GreenMail docs](https://greenmail-mail-test.github.io/greenmail/)
- Mail can be preloaded from a directory of `.eml` files with
  `-Dgreenmail.preload.dir`, and `-Dgreenmail.auth.disabled` accepts any
  password. [GreenMail docs](https://greenmail-mail-test.github.io/greenmail/)
- **IDLE is supported.** `IdleCommand` attaches a folder listener and sends
  `EXISTS`/`EXPUNGE` while idling. The capability string includes `IDLE`.
  [source: IdleCommand.java](https://github.com/greenmail-mail-test/greenmail/blob/master/greenmail-core/src/main/java/com/icegreen/greenmail/imap/commands/IdleCommand.java),
  [CapabilityCommand.java](https://github.com/greenmail-mail-test/greenmail/blob/master/greenmail-core/src/main/java/com/icegreen/greenmail/imap/commands/CapabilityCommand.java)
- **XOAUTH2 is supported.** It advertises `SASL-IR AUTH=XOAUTH2` and checks the
  bearer token as the user's password. The release-2.1.15 tag has it too.
  [source: AuthenticateCommand.java](https://github.com/greenmail-mail-test/greenmail/blob/master/greenmail-core/src/main/java/com/icegreen/greenmail/imap/commands/AuthenticateCommand.java)
- **No Gmail extensions.** The capability list is `LITERAL+ UIDPLUS SORT IDLE
  MOVE SASL-IR AUTH=XOAUTH2` (plus `QUOTA`), with no `X-GM-EXT-1`. (Source,
  same file as above.)
- New mail during IDLE needs a second actor: an SMTP delivery or an IMAP
  `APPEND` from the test. A dropped connection or a crafted `NO`/`BAD` cannot be
  scripted without a proxy in front.
- It needs a JVM or Docker. With Docker, `testcontainers` (0.28) starts a
  `GenericImage` through `AsyncRunner`/`SyncRunner`.
  [docs.rs: testcontainers](https://docs.rs/testcontainers/latest/testcontainers/)

### Option B: Dovecot in a container

- Supports `oauthbearer` and `xoauth2`, but token validation has to be
  configured: an introspection URL (`oauth2_introspection_mode` =
  `auth`/`get`/`post`) or local JWT validation with signing keys
  (`oauth2_introspection_mode = local`, `oauth2_local_validation`).
  [Dovecot: OAuth2](https://doc.dovecot.org/main/core/config/auth/databases/oauth2.html)
- It is a production server, with no Gmail extensions and no scripted faults.
  It needs the same Docker dependency as GreenMail, and more configuration, for
  less control. It is useful only as an extra RFC-conformance check, and
  GreenMail already fills that role at lower cost.

### Option C: our own scriptable fake on `imap-codec` / `imap-next`

- `imap-codec` parses and serializes IMAP4rev1 on top of `imap-types`. It is
  described as a building block for client and server implementations.
  [docs.rs: imap-codec README](https://docs.rs/crate/imap-codec/2.0.0-alpha.8/source/README.md)
- `imap-next` (0.3.4) is a sans-I/O layer over IMAP's protocol flows: literals,
  AUTHENTICATE and IDLE. Its `server` module has a `Server` whose `Event`s
  include `CommandReceived`, `CommandAuthenticateReceived`,
  `AuthenticateDataReceived`, `IdleCommandReceived` and `IdleDoneReceived`, and
  `authenticate_continue`/`authenticate_finish` to drive SASL.
  [docs.rs: imap-next](https://docs.rs/imap-next),
  [server::Event](https://docs.rs/imap-next/latest/imap_next/server/enum.Event.html)
- No ready-made mock IMAP server crate turned up. The fake would be our own
  test-support code: a tokio `TcpListener` on `127.0.0.1:0`, one `imap-next`
  `Server` per connection, and a scenario object that holds the mailbox and a
  control channel the test uses to push `EXISTS`, drop the socket, or answer
  `NO`.
- With full control, it can advertise `X-GM-EXT-1`, return `X-GM-MSGID` and
  `X-GM-THRID`, accept or reject any XOAUTH2 token, and reproduce any fault
  case. The cost is that the fake encodes our reading of Gmail's behaviour and
  can drift from it.
- In-process or spawned: the fake runs as a server task inside the test
  process, and the Postal binary under test connects to it over TCP. Postal and
  the fake still meet at the real wire protocol, so this satisfies "only
  external processes are faked". No extra binary or container is needed.

### Driving the TUI

- **ratatui `TestBackend`**: renders into an in-memory buffer. It has `new`,
  `buffer`, `assert_buffer`, `assert_buffer_lines` and `resize`, and implements
  `Display`. [docs.rs: TestBackend](https://docs.rs/ratatui/latest/ratatui/backend/struct.TestBackend.html)
  The ratatui testing recipe pairs it with `insta::assert_snapshot!` (text only,
  no colours) and states that `TestBackend` does not cover the event loop, key
  handling, terminal setup and teardown, or exit codes. For those it points to a
  pty harness. [ratatui: snapshot testing](https://ratatui.rs/recipes/testing/snapshots/)
  `TestBackend` calls Postal's own render code in-process, so under our test
  rule it is a unit test through an internal seam.
- **`termlens` (0.11.10)**: a headless pty harness that runs the compiled
  binary. `bin!` (`CARGO_BIN_EXE_<name>`) starts it with a fixed grid, a cleared
  environment and a deadline. The API has `.env(...)`, `send(Key::...)`,
  `send_str`, `wait_until(|screen| ...)`, `wait_exit()`, and
  `assert_screen_snapshot!` (insta, with styles). Timeout errors embed the
  screen. [docs.rs: termlens](https://docs.rs/termlens/latest/termlens/)
  The 0.4 docs say it is Unix-only, tested on Linux and macOS in CI, which
  matches Postal's platforms. [docs.rs: termlens 0.4.0](https://docs.rs/crate/termlens/0.4.0)
  Its dependencies are `portable-pty` and `vt100`. Alternatives in the same
  space are `testty` and `term-transcript`.
- A pty test with `bin!` is the external boundary in the strict sense: the user's
  terminal on one side and the IMAP socket on the other.

### Wiring the binary to the fake

These follow from #1 and still need decisions:

- **Config:** Postal reads `~/.config/postal/config.toml`. Tests need to point
  `HOME`/`XDG_CONFIG_HOME` at a temp dir and write a config naming
  `127.0.0.1:<port>`. termlens clears the environment, so the test sets these
  explicitly.
- **TLS:** Gmail requires TLS, while the fake listens in plaintext. Either the
  config allows a plaintext or STARTTLS endpoint (accepted for loopback only),
  or the fake serves TLS with a generated certificate that the test trusts.
- **Secrets:** The OS keychain cannot run in CI. The `0600` file fallback from
  #1 must be selectable through config or the environment, so tests use it.
- **Cache:** It goes in the temp dir as well, which also covers "Cache shown at
  start" as a two-run scenario.

## Recommendation

1. **Fake IMAP server: build a scriptable in-test fake on `imap-next` +
   `imap-codec`** as test-support code under `tests/`. Bind it to
   `127.0.0.1:0`, give each test a scenario with a mailbox and a control
   channel (push `EXISTS`, drop the connection, reply `NO`/`BAD`, expire a
   token), and advertise `IDLE`, `AUTH=XOAUTH2` and `X-GM-EXT-1`. It is the
   only option that covers Gmail extensions, connection drops and error
   injection without a container.
2. **Optional GreenMail conformance job:** run a small smoke suite (login over
   XOAUTH2, list the INBOX, IDLE wakes on an `APPEND`) against
   `greenmail/standalone` through `testcontainers`, behind a cargo feature or
   in CI only. A real server catches drift in the fake's RFC behaviour, and
   GreenMail already supports IDLE and XOAUTH2. Skip Dovecot: same Docker cost,
   more configuration.
3. **TUI: drive the real binary through `termlens`** with `insta` snapshots and
   `wait_until`, never sleeps. Keep each scenario short and synthetic, one
   behaviour per test. Do not write `TestBackend` render tests: they
   are in-process unit tests, which `AGENTS.md` forbids, and they miss the event
   loop and key handling.
4. **Settle before implementation:** a loopback TLS or plaintext endpoint in
   config, a forced file-secret backend, and `XDG_CONFIG_HOME`/cache paths that
   tests can redirect. These belong in the config and onboarding tickets of #1.
