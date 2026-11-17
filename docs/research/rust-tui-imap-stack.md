# Rust crates for the TUI and IMAP stack

Research for issue #3 (parent: map #1). Facts are as of 2026-11-17, taken
from crates.io metadata, the crates' source and changelogs, and Google's
Gmail IMAP documentation. Each claim names its source; "source" means the
published crate or repository at the version named.

## Question

Which crates should Postal build on for the terminal UI, IMAP and the async
runtime, and how does one event loop combine terminal input, IDLE and timers?
Allocation behaviour is noted where the source shows it.

## TUI

### Candidates

| Crate | Latest | Last release | Notes |
|---|---|---|---|
| `ratatui` | 0.30.2 | 2026-06-19 | Repo pushed 2026-11-17, 23k stars; 0.30 split into `ratatui-core` and backend crates |
| `crossterm` | 0.29.0 | 2025-04-05 | Repo active (commits 2026-11-13); ratatui's default backend |
| `termion` | 4.0.6 | 2025-11-21 | Unix only, Redox GitLab; ratatui backend exists |
| `termwiz` | 0.23.3 | 2025-03-20 | Part of the wezterm monorepo; ratatui backend exists |
| `cursive` | 0.21.1 | 2024-08-03 | Retained-mode widget toolkit, slower release cadence |

Sources: crates.io API (`/api/v1/crates/<name>`), GitHub API for push dates.

Ratatui with crossterm is the mainstream choice and the only pair with both an
active release line and an active repository. Termion would drop nothing we
need (Windows is out of scope) but has a smaller ecosystem; cursive's
retained-mode model fits form-style apps, not a lazygit-like list/preview view.

### Ratatui rendering model

Ratatui is immediate mode: every `Terminal::draw` call renders all widgets
into an in-memory `Buffer`, then diffs it against the previous frame's buffer
and writes only changed cells
([ratatui.rs, "Rendering under the hood"](https://ratatui.rs/concepts/rendering/under-the-hood/)).

What the source (`ratatui-core` 0.1.2, `ratatui-crossterm` 0.1.2) shows about
allocation per frame:

- **Two buffers, reused.** `Terminal` holds `buffers[2]`; `swap_buffers`
  calls `Buffer::reset`, which resets each `Cell` in place
  (`src/terminal/buffers.rs`, `src/buffer/buffer.rs`). No buffer is
  reallocated per frame; only a resize reallocates.
- **Diff is an iterator.** `Terminal::flush` feeds `diff_iter` straight into
  `Backend::draw`; the `Vec`-returning `Buffer::diff` is a convenience not used
  on the draw path (`src/terminal/buffers.rs`).
- **Cell symbols are inline.** `Cell::symbol` is `Option<CompactString>`, which
  stores strings up to 24 bytes inline, so a grapheme never hits the heap
  (`src/buffer/cell.rs`, ratatui PR #601).
- **Text can borrow.** `Span::content` is `Cow<'a, str>`; `Line` holds
  `Vec<Span>` and `Text` holds `Vec<Line>` (`src/text/*.rs`). Building a
  `Line` allocates its `Vec`, but span text can borrow from the Cache or the
  rendered Preview instead of being copied each frame.
- **Output is queued.** The crossterm backend `queue!`s cursor moves and style
  changes into its writer and flushes once (`ratatui-crossterm` `lib.rs`).
  Wrap stdout in a `BufWriter` if the writer is unbuffered.

Practical consequence: the per-frame cost is dominated by what Postal's own
widgets allocate. Rendering only the visible window of the Message List from
borrowed data, and keeping the rendered Preview as owned `Text<'static>` built
once per Message, keeps frames close to allocation-free.

### Terminal input

Crossterm's `event-stream` feature provides `EventStream`, a
runtime-agnostic `futures::Stream` of key/resize events. When no event is
ready it hands the waker to one helper thread that blocks in the internal
poll (`crossterm` 0.29.0 `src/event/stream.rs`). That is one extra OS thread
per stream, not per poll.

## IMAP

### Candidates

| Crate | Latest stable | Last release | Model | Status |
|---|---|---|---|---|
| `async-imap` | 0.12.0 | 2026-09-30 | async, futures-io | Active (chatmail; commits 2026-11-09) |
| `imap` (rust-imap) | 2.4.1; 3.0.0-alpha.15 | 2025-02-08 | blocking | Only dependabot commits in 2026; 3.0 in alpha since 2021 |
| `imap-codec` / `imap-types` | 1.0.0; 2.0.0-alpha.10 | 2026-11-02 | sans-I/O parser/serializer | Active, 2.0 still alpha |
| `imap-next` | 0.3.4 | 2026-02-18 | sans-I/O flows (literals, AUTHENTICATE, IDLE) | Last commit 2026-02-18 |
| `imap-client` (pimalaya) | 0.3.1 | 2026-05-17 | async over imap-next | Archived, "moved to io-imap" |
| `io-imap` (pimalaya) | 0.11.2 | 2026-11-15 | sans-I/O coroutines + blocking client | First release 2026-06-03; 0.10 → 0.11.2 in three days |

Sources: crates.io API, GitHub API, each repo's README and CHANGELOG.

### Feature check

| Need | async-imap 0.12 | imap 2.4/3.0α | imap-codec + imap-next | io-imap 0.11 |
|---|---|---|---|---|
| IDLE | `extensions::idle::Handle`; `wait()` re-issues at 29 min, returns a `StopSource` to interrupt | yes | IDLE flow in imap-next | yes, plus a `watch` API |
| XOAUTH2 | `Client::authenticate("XOAUTH2", ..)` with an `Authenticator` (doc example in `client.rs`) | `Authenticator` | `AuthMechanism::XOAuth2` | SASL XOAUTH2, OAUTHBEARER |
| Gmail `X-GM-*` | `Fetch::gmail_labels()`, `gmail_msg_id()` (0.11.2); parser (`imap-proto` 0.17) also parses `X-GM-THRID` | via imap-proto, fewer accessors | no `X-GM-*` types in imap-types | not first-class |
| CONDSTORE | `select_condstore`, `Mailbox::highest_modseq`, `Fetch::modseq` | partial | `ext_condstore_qresync` feature | CONDSTORE + QRESYNC |
| Runtime | `runtime-tokio` or `runtime-async-std` (default) | none (threads) | any | blocking or bring your own |

Sources: `async-imap` 0.12.0 `CHANGELOG.md`, `Cargo.toml`, `src/client.rs`,
`src/extensions/idle.rs`, `src/types/fetch.rs`; `imap-proto` `src/gmail.rs`;
`imap-codec` `Cargo.toml` features; `io-imap` README and `Cargo.toml`.

### Allocation in async-imap

- The stream reads into a `BytesMut`-backed block buffer that is grown, reset
  and reused across responses (`src/imap_stream.rs`).
- Parsed responses are `imap-proto` types borrowing `Cow<'_, str>` from that
  block; `Fetch` keeps the owning block alive with `self_cell`, so field
  access (envelope, body, labels) does not copy (`src/types/fetch.rs`,
  `src/types/response_data.rs`).
- Each response owns its block, so a large `BODY[]` stays one allocation
  until the `Fetch` is dropped. Fetching `BODY.PEEK[]` per Message on demand,
  not in bulk, bounds peak memory.

### Gmail specifics

- XOAUTH2 initial response is
  `base64("user=" user "\x01auth=Bearer " token "\x01\x01")`; scope
  `https://mail.google.com/`
  ([Google: OAuth 2.0 Mechanism](https://developers.google.com/workspace/gmail/imap/xoauth2-protocol)).
- `X-GM-EXT-1` in CAPABILITY announces `X-GM-RAW`, `X-GM-MSGID`, `X-GM-THRID`,
  `X-GM-LABELS`
  ([Google: IMAP Extensions](https://developers.google.com/workspace/gmail/imap/imap-extensions)).
  `X-GM-MSGID` gives a stable per-message key for the Cache beyond UIDVALIDITY.
- Google's extension page shows IDLE in its sample CAPABILITY but does not
  document CONDSTORE or QRESYNC. Treat CONDSTORE as optional: check
  CAPABILITY at runtime and fall back to a UID range fetch.
- Gmail caps IMAP connections per account at 15 (secondary sources quoting
  Google, e.g. Atlassian KB; no primary page found). Postal should hold at
  most two: one in IDLE, one for fetches.

## Async runtime

| Runtime | Latest | Status |
|---|---|---|
| `tokio` | 1.54.1 (2026-10-21) | Active |
| `smol` | 2.0.2 (2024-09-07) | Repo active (2026-10-26), stable API |
| `async-std` | 1.13.2 (2025-08-15) | **Discontinued**; README says "use smol instead" |

Source: crates.io API; `async-rs/async-std` README.

async-imap's default feature is `runtime-async-std`, so Postal must use
`default-features = false, features = ["runtime-tokio"]` (or bring its own
smol-compatible stream). Tokio is the best-supported option for async-imap,
TLS (`tokio-rustls`) and the OAuth loopback server; smol would work but adds
compat shims. Plain threads with the blocking `imap` crate are viable but
its 3.0 has been alpha for years and IDLE then needs a dedicated thread per
connection plus channels, which is what async gives for free.

A `current_thread` Tokio runtime is enough: the workload is two sockets and a
terminal, and it avoids work-stealing overhead and `Send` bounds on UI state.

## Event loop shape

```text
                 ┌────────────── tokio current_thread ──────────────┐
 crossterm  ───► │ select! {                                         │
 EventStream     │   ev   = events.next()   => handle key / resize   │
 IDLE task  ───► │   msg  = imap_rx.recv()  => apply to Message List │ ─► draw
 (own conn)      │   _    = tick.tick()     => spinners, timeouts    │
                 │ }                                                 │
                 └───────────────────────────────────────────────────┘
```

- **IDLE in its own task and connection.** `Session::idle(self)` moves the
  session into the `Handle` until `done()`, so commands cannot share it. The
  task loops: `init` → `wait` → on `NewData`/`Timeout`, `done` and fetch
  changes → send a compact event (UIDs, not bodies) over a bounded `mpsc`.
  On error, reconnect with backoff (#1: "reconnect on drop").
- **Fetch connection** serves on-demand body fetches for the Preview,
  answering over a `oneshot` per request.
- **UI task** owns all state and redraws only after an event changed it, so
  idle CPU is zero and frames are not rendered on a fixed timer. The tick
  exists only while something animates.
- `tokio::select!` is a macro over futures on the stack; it does not
  allocate. Channel sends of small `Copy`/`Vec<u32>` events are the only
  per-event allocation.

## Recommendation

1. **TUI: `ratatui` 0.30 + `crossterm` 0.29** with the `event-stream`
   feature. Render the Message List from borrowed Cache data with `Cow`
   spans; build Preview `Text` once per Message.
2. **IMAP: `async-imap` 0.12** with `default-features = false,
   features = ["runtime-tokio"]`. It is the only maintained crate that has
   IDLE, XOAUTH2, CONDSTORE and Gmail `X-GM-LABELS`/`X-GM-MSGID` accessors in
   a stable release, with zero-copy parsing via `imap-proto`.
3. **Runtime: Tokio, `current_thread`**, one `select!` loop over terminal
   events, an IMAP event channel and an optional tick; IDLE and fetch each on
   their own connection.
4. **Watch list, not now:** `io-imap` (CONDSTORE+QRESYNC, sans-I/O) once its
   API stops changing weekly; `imap-codec` 2.0 once it leaves alpha.
   Avoid `imap` (sync) 3.0-alpha and anything on `async-std`.
5. Hide the IMAP crate behind a Postal-owned trait so a later swap to
   `io-imap` touches one module, and the integration tests fake the server,
   not the crate.
