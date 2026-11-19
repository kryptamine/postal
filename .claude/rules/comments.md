---
description: How to write comments, and what never to comment
---

# Comments

These rules cover every kind of comment, in Rust (`//`, `///`, `//!`) and
shell alike.

## Rules

- Comment only a declaration or block whose intent is not obvious at a glance.
  Obvious code gets none, public or not; a comment that only expands the
  identifier's name is worse than silence.
- **A comment is at most three lines.** A hard cap. A longer explanation belongs
  in `docs/adr/`, with one line in the code pointing at it.
- Explain the gist of the thing the comment sits on: what it does, plus the why
  when that is not obvious. Call sites move, so leave out who calls it and when.
- A measured value is worth stating. Its provenance (which issue, session
  or commit) lives in `git log` and `docs/adr/`.
- State a contract where it is relied on, rather than where a caller might one
  day lean on it.
- Keep a comment that stops a reader "fixing" the code and breaking it: a
  measured constant, a constraint the protocol or server imposes, an ordering that matters,
  a case that looks unhandled and is not.
- Plain words. If a sentence needs a second read, rewrite it shorter.
- An `#[expect(...)]` says why with `reason = "..."`.
- In tests, a comment says what the case protects: the behaviour that would
  otherwise be lost, not what the assertion does.
- A field, method or constant deleted takes its comment with it.

## Examples

Bad, restates the code and names the caller:

```rust
// Called by sync_mailbox on every tick.
// Parses the envelope from the raw message.
pub fn parse_envelope(body: &str) -> Option<Envelope> { ... }
```

Good, what the signature cannot state:

```rust
/// Returns `None` when the message has no parsable headers.
pub fn parse_envelope(body: &str) -> Option<Envelope> { ... }
```

Good, stops a reader "fixing" it:

```rust
// The IMAP server on this host serves a self-signed certificate,
// and this client only ever talks to the configured host above.
.danger_accept_invalid_certs(true)
```

Good, none at all:

```rust
fn header(name: &str, value: &str) -> String { ... }
```

## Comment pass

Write the code first. As the last step before reporting, check the comments this
change added or touched against the rules above, editing comments only.
