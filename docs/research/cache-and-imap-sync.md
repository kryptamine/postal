# Research: on-disk Cache storage and incremental IMAP sync

Ticket: [#5](https://github.com/kryptamine/postal/issues/5), part of map [#1](https://github.com/kryptamine/postal/issues/1).
Researched 2026-11-17.

Question: what should store the Cache, and how does it stay in sync with the
Inbox incrementally? The settled constraints are one Account, `INBOX` only,
read-only access, IMAP `IDLE`, a Cache that is shown at start and synced in the
background, and Gmail first. Performance matters.

## 1. Storage candidates

### What the Cache holds

- **Mailbox state**: `UIDVALIDITY`, `UIDNEXT`, `HIGHESTMODSEQ` (if CONDSTORE is
  available), and the ordered list of every Inbox UID, which is needed to map
  `EXPUNGE` sequence numbers (see §2.6).
- **Message List rows**: UID, `X-GM-MSGID`, flags, internal date, and envelope
  fields (from, subject, date). Each row is a few hundred bytes.
- **Bodies**: the raw or rendered Preview source per Message, kilobytes to
  megabytes each. Fetched on demand or prefetched.

At start, the hot read is "the newest ~100 rows in date/UID order". Writes
come in small batches from sync and `IDLE`.

### Comparison

| | SQLite (`rusqlite` 0.43, `bundled`) | redb 4.6 | fjall 3.1 | Flat files |
|---|---|---|---|---|
| Model | Relational, B-tree, SQL, secondary indexes | Typed KV, copy-on-write B+trees, MVCC | KV, LSM tree (RocksDB-like), keyspaces | One file per Message plus an index file |
| Crash safety | Atomic commit; WAL mode lets readers and the writer run concurrently [S1] | "Fully ACID-compliant", "Crash-safe by default" [S2] | Writes reach OS buffers only; durable after `persist(PersistMode::SyncAll)`; no formal crash guarantee in the README [S3] | Hand-rolled: write-temp + `fsync` + `rename`, and keeping index and files consistent is our job |
| Startup read | One `open`, one indexed range query | One `open`, range scan on a UID key | One `open`; LSM reads touch several levels | `open` per file; SQLite measured ~35% faster than individual files for ~10 KB blobs [S4] |
| Write cost | Each commit costs an fsync; with `synchronous=NORMAL` in WAL mode, only checkpoints sync [S1] | Fastest "individual writes" in its own benchmark: redb 920 ms vs SQLite 7040 ms [S2] | Fastest batch writes in redb's benchmark (353 ms) [S2] | One file write per Message, plus index rewrites |
| Format stability | Stable, documented file format | "The file format is stable" [S2] | Breaking changes come with a major version and a migration path [S3] | Ours |
| Binary size delta (measured) | **+1.70 MiB** | **+0.66 MiB** | **+0.82 MiB** | ~0 |
| Pure Rust | No (compiles bundled C via `cc`) | Yes | Yes | Yes |
| Schema evolution | `PRAGMA user_version` plus `ALTER TABLE` | Manual table versioning | Manual | Manual |
| Inspectability | `sqlite3` CLI | None out of the box | None | `ls`/`cat` |

Binary size was measured on 2026-11-17 on macOS arm64 with Rust 1.94. Each
binary is a release build (`lto = true`, `codegen-units = 1`, `strip = true`)
that opens a store, writes one row and reads it back. Against a 296 KiB
baseline that does the same with `std::fs`, the binaries came out at
`rusqlite`+`bundled` 2.00 MiB, redb 0.97 MiB and fjall 1.14 MiB. The deltas
are an upper bound on what each library adds to Postal, since some of the code
is shared with dependencies Postal pulls in anyway.

redb's README benchmark covers redb, lmdb, rocksdb, fjall and SQLite on a
Ryzen 9950X3D with NVMe [S2]. For random reads, redb took 1138 ms, fjall
2177 ms and SQLite 4283 ms. That is millions of lookups, so the per-lookup
cost is in microseconds for all three. At the startup read size (~100 rows),
none of them is the bottleneck: the filesystem cache and the terminal draw
cost more. **Startup latency does not decide between them.**

### Reading of the comparison

- **Flat files** lose. Crash-consistent writes, the index and the space
  overhead (padding to block size, 20% larger than SQLite in [S4]) would all
  be reimplemented by hand. Maildir-style layouts buy interoperability, which
  Postal does not need.
- **fjall** is built for write-heavy workloads. The Cache is read-heavy with
  small, rare writes, and its default durability is weaker, so it has no
  advantage here.
- **redb** is the strongest pure-Rust option: ACID by default, the smallest
  delta, fast point writes, and UID-ordered keys give the Message List order
  for free. Its cost is that every secondary index (for example `X-GM-MSGID` →
  body) and every schema migration is written by hand.
- **SQLite** costs about 1 MiB more than redb and a C toolchain at build time
  (`bundled`). In exchange it gives indexes, ordering by date, migrations via
  `user_version`, and a CLI for debugging users' caches. Postal is read-only,
  so a transaction lost after a power cut is simply fetched again. That makes
  WAL plus `synchronous=NORMAL` safe, and removes the per-commit fsync that
  dominates SQLite's write cost [S1].

## 2. IMAP sync primitives

### 2.1 UIDs and UIDVALIDITY

UIDs are "assigned in a strictly ascending fashion in the mailbox". When a
message is expunged, "its UID MUST NOT be reused under the same UIDVALIDITY
value" (RFC 9051 §2.3.1.1 [S5]; RFC 3501 §2.3.1.1 says the same [S6]). The
client must compare the server's UIDVALIDITY with the cached value. If it
changed, every cached UID for that mailbox is invalid (RFC 4549 §4.1 [S7]).
Under CONDSTORE, the client "MUST delete the cached value of HIGHESTMODSEQ" as
well (RFC 7162 §3.1.2.1 [S8]).

`UIDNEXT` is the next UID to be assigned. If the cached value equals the
server's value, there are no new arrivals since the last sync.

### 2.2 New messages

`UID FETCH <lastseenuid+1>:* <items>` (RFC 4549 §4.3.1 [S7]). One trap: a
range `n:*` with `n` above every UID still matches the highest-UID message
(RFC 3501 §6.4.8 [S6]). The client must drop results with
`uid <= lastseenuid`, or skip the command when `UIDNEXT` is unchanged.

### 2.3 Flag changes

- Without CONDSTORE: `UID FETCH 1:<lastseenuid> FLAGS` over the cached range
  (RFC 4549 §4.3.1 [S7]). The cost is linear in the cached count.
- With CONDSTORE: cache `HIGHESTMODSEQ` from `SELECT`/`EXAMINE` and ask only
  for changes, as in `UID FETCH 1:* (FLAGS) (CHANGEDSINCE <modseq>)` (RFC 7162
  §3.1.2.1, §3.1.4.1 [S8]; RFC 4549 §6.1 [S7]). If `HIGHESTMODSEQ` is
  unchanged, nothing changed at all.

### 2.4 Server-side deletions

- `CHANGEDSINCE` does **not** report expunges. A CONDSTORE-only client needs an
  extra round trip (RFC 7162 §3.2.1 [S8]).
- Without QRESYNC, send `UID SEARCH UID 1:<lastseenuid>`, a lower-traffic
  substitute for fetching flags (RFC 4549 §4.3.1 note 3 [S7]; RFC 7162 §3.2.6
  example 1 [S8]). Any cached UID missing from the result was expunged.
- With QRESYNC, `SELECT INBOX (QRESYNC (<uidvalidity> <modseq> [known-uids]))`
  returns `VANISHED (EARLIER) <uids>` plus the changed flags in one step
  (RFC 7162 §3.2.5, §3.2.10.1 [S8]).
- In Gmail, archiving or deleting a Message removes the Inbox label. In IMAP
  terms the Message is expunged from `INBOX`, so it shows up through the same
  detection.

### 2.5 What Gmail supports

- A live probe of `imap.gmail.com:993` on 2026-11-17 returned this
  pre-authentication `CAPABILITY`:
  `IMAP4rev1 UNSELECT IDLE NAMESPACE QUOTA ID XLIST CHILDREN X-GM-EXT-1 XYZZY SASL-IR AUTH=XOAUTH2 AUTH=PLAIN AUTH=PLAIN-CLIENTTOKEN AUTH=OAUTHBEARER`.
  It is IMAP4rev1, not IMAP4rev2, and it lists **no QRESYNC**.
- Google's extension docs do not mention CONDSTORE or QRESYNC at all [S9]. Its
  example post-login capability list also has neither. **Gmail is widely
  reported to add CONDSTORE after login, but we could not confirm this from a
  primary source without credentials.** Treat both as optional: read the
  post-login `CAPABILITY` and use CONDSTORE only if it is listed. Assume no
  QRESYNC.
- `X-GM-EXT-1` provides [S9]:
  - `X-GM-MSGID`: a 64-bit ID, "a unique message ID for each email ... across
    multiple folders", the same ID that the web UI and the Gmail API use. The
    docs say nothing about whether it stays stable over time.
  - `X-GM-THRID`: a 64-bit thread ID that matches web UI threading.
  - `X-GM-LABELS`: the labels on the Message.
  - `X-GM-RAW`: Gmail search syntax inside `SEARCH`.
- Consequence: a Message that leaves `INBOX` and comes back gets a new UID,
  because UIDs are never reused (§2.1). It keeps its `X-GM-MSGID`. Keying
  bodies by `X-GM-MSGID` therefore saves refetching the body. It also lets
  bodies outlive a UIDVALIDITY reset.
- `X-GM-THRID` and `X-GM-LABELS` have no reader in the MVP: there are no
  threads and labels are not shown. Under the type rule in `AGENTS.md`, do not
  fetch or store them yet.

### 2.6 IDLE

- The server pushes untagged `EXISTS`, `EXPUNGE <seq>` and `FETCH (FLAGS ...)`
  while the client is idling. The client re-issues `IDLE` at least every 29
  minutes (RFC 2177).
- `EXPUNGE` carries a **sequence number**, not a UID, and every later message's
  sequence number drops by one (RFC 3501 §7.4.1 [S6]). To map it back to a UID,
  the client keeps the full, ordered Inbox UID list in memory, and the Cache
  persists it. Without QRESYNC, nothing turns this into `VANISHED` UIDs.

### 2.7 Rust IMAP crates (source read at the versions listed)

- `imap-proto` 0.17.0 parses `HIGHESTMODSEQ`, `UIDNEXT` and `UIDVALIDITY`
  response codes, the `MODSEQ` fetch attribute, `VANISHED [(EARLIER)]`, and
  `X-GM-MSGID`/`X-GM-THRID`/`X-GM-LABELS` (`src/rfc4551.rs`, `src/rfc7162.rs`,
  `src/gmail.rs`).
- `async-imap` 0.12.0 has `Mailbox { uid_next, uid_validity, highest_modseq,
  exists, .. }`, `Session::select_condstore`, `uid_fetch`, `uid_search` and
  `idle()`. `Fetch` exposes `uid`, `modseq`, `gmail_msg_id()` and
  `gmail_labels()`, but has no accessor for `X-GM-THRID`.
- `async-imap` has no QRESYNC or ENABLE helper and no `EXAMINE (CONDSTORE)`.
  Those would go through `run_command` plus `read_response`.
- `async-imap` defaults to async-std; `runtime-tokio` is a feature flag.

## 3. Recommendation

**Storage: SQLite via `rusqlite` with `bundled`, in WAL mode with
`synchronous=NORMAL`.** Flat files are rejected and fjall is a poor fit. redb
is the documented fallback if binary size or a pure-Rust build becomes a hard
requirement; it saves about 1 MiB.

- SQLite wins on what the Cache actually needs: indexed ordering, a second key
  (`X-GM-MSGID`), cheap schema migrations (`user_version`) and inspectability.
  The ~1 MiB cost is small next to a TLS stack.
- `synchronous=NORMAL` is safe because the server is the source of truth. A
  lost commit is refetched on the next sync.
- Proposed tables:
  - `mailbox(uidvalidity, uidnext, highestmodseq)`
  - `message(uid PRIMARY KEY, gm_msgid, internal_date, flags, from, subject, …)`
    with an index on `internal_date`
  - `body(gm_msgid PRIMARY KEY, raw)`
- Only add the columns that the Message List and the Preview read.

**Sync algorithm** (each step runs in the background after the cached Message
List is drawn):

1. Open the Inbox read-only with `EXAMINE INBOX`. If CONDSTORE is listed, add
   `(CONDSTORE)`.
2. If UIDVALIDITY changed, drop the `message` rows and the mailbox state. Keep
   `body`, which is keyed by `X-GM-MSGID`.
3. **New mail**: if `UIDNEXT` changed, run `UID FETCH <last+1>:*` with
   `(UID FLAGS INTERNALDATE ENVELOPE X-GM-MSGID)` and filter
   `uid > last`.
4. **First run or empty Cache**: fetch only the newest N Messages (for example
   500, by sequence range `EXISTS-N+1:*`) and backfill older ones lazily on
   scroll. A Gmail Inbox can hold tens of thousands of Messages.
5. **Deletions**: run `UID SEARCH UID <min>:<last>` and drop the cached UIDs
   that are missing from the result. A cheap pre-check: when the cached count
   plus the new arrivals equals `EXISTS`, nothing was expunged and the search
   can be skipped.
6. **Flags**: with CONDSTORE, run `UID FETCH 1:* (FLAGS) (CHANGEDSINCE <m>)`
   and skip it when `HIGHESTMODSEQ` is unchanged. Without it, run
   `UID FETCH <min>:<last> FLAGS` over the cached window.
7. Write everything in one transaction, then enter `IDLE`.
8. **During IDLE**: on `EXISTS`, run step 3. On `EXPUNGE n`, remove the n-th
   UID from the in-memory ordered list and the Cache. On `FETCH FLAGS`, update
   the row. Re-issue `IDLE` before 29 minutes. After a reconnect, rerun steps
   1–7.
9. Fetch bodies with `BODY.PEEK[]` on demand, keyed by `X-GM-MSGID`. This is
   needed even though `EXAMINE` already prevents `\Seen` from being set.

**Do not depend on QRESYNC** (Gmail does not advertise it). Treat CONDSTORE as
an optimisation, not a requirement. Before locking the design, confirm the
post-login `CAPABILITY` against a real Gmail Account.

## Sources

- [S1] SQLite, "Write-Ahead Logging": https://www.sqlite.org/wal.html
- [S2] redb README (benchmarks, guarantees): https://github.com/cberner/redb
- [S3] fjall README: https://github.com/fjall-rs/fjall
- [S4] SQLite, "35% Faster Than The Filesystem": https://www.sqlite.org/fasterthanfs.html
- [S5] RFC 9051, IMAP4rev2: https://www.rfc-editor.org/rfc/rfc9051
- [S6] RFC 3501, IMAP4rev1: https://www.rfc-editor.org/rfc/rfc3501
- [S7] RFC 4549, Synchronization Operations for Disconnected IMAP4 Clients: https://www.rfc-editor.org/rfc/rfc4549
- [S8] RFC 7162, IMAP CONDSTORE and QRESYNC: https://www.rfc-editor.org/rfc/rfc7162
- [S9] Google, Gmail IMAP Extensions: https://developers.google.com/workspace/gmail/imap/imap-extensions
- RFC 2177, IMAP4 IDLE command: https://www.rfc-editor.org/rfc/rfc2177
- Crate sources: `imap-proto` 0.17.0 (https://github.com/djc/imap-proto),
  `async-imap` 0.12.0 (https://github.com/async-email/async-imap), versions as
  of crates.io on 2026-11-17.
