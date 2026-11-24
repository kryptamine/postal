# Every Message's body is indexed, in the background

A Search matched only what the Cache held of a Message: sender, subject,
Snippet and recipients (ADR 0023). A word past the Snippet found nothing,
though the server had it. Desktop clients that search everything, notmuch,
Thunderbird, Apple Mail, download the mailbox and index it themselves;
Postal does the same, so a Search stays in process (ADR 0007), matches
bodies, and works offline. Issue #157.

## The body pass

The fetch connection indexes bodies as `SpareWork`, taking turns with the
pass over envelopes once the pass over Sent is done:

- A batch is the 20 newest Messages whose body the index lacks, all in the
  Folder of the newest one, outside Trash and Spam. Their text parts (ADR
  0024) are fetched with `BODY.PEEK[n]<0.32768>`: past 32 KiB a body is
  mostly quoted history. Attachments are never fetched.
- Each batch is one Cache write, in its own transaction, so the sync
  connection's writes never wait long behind it.
- A request (a Preview, a Reply, a Backfill) waits on one batch at most:
  the queue is read after every batch, and requests go first.
- A Message still without a Snippet gets it cut from the same fetch.
- Once no body is left, the pass looks again every 5 seconds, for mail
  IDLE brought, a Backfill or a resync wrote.
- A server refusing a fetch (`NO`, as Gmail's `[OVERQUOTA]`) ends the pass
  until that recheck.

## The index

A body is the seventh column of the contentless `message_text`, matched by
a word without an operator. Neither its text nor the raw body is kept: the
raw bodies stay the Preview's ~200 MB LRU (ADR 0005). So an index entry
cannot be rewritten with its body, and `message.bodied` says whether the
entry holds it:

- Any rewrite of the entry (a new row, a changed Snippet or sender, a Move,
  a UIDVALIDITY change) leaves it without the body and `bodied` at 0, and
  the pass fetches the body again.
- A Message Gmail files under several labels shares one body key (ADR 0005),
  and each copy is indexed from one fetch.
- `message_unbodied`, a partial index, finds the next batch without
  reading the indexed rows.

The `prefix` indexes now hold one and two characters, not up to four: a
word of three or more is a range scan of the terms.

## Measured

`sqlite3` on the reference machine: 500,000 synthetic Messages as in ADR
0023, words of a dictionary at Zipf frequencies, each body as many words as
one of 117 real bodies sampled (mean 290 words, 1.3 KB of synthetic text,
about 3.1 KB of real), reading the newest 500 candidates:

| Index | Size | Build | Slowest Query |
| --- | --- | --- | --- |
| no bodies, `prefix = '1 2 3 4'` (before) | 614 MB | 35 s | 0.5 ms |
| bodies, `prefix = '1 2 3 4'` | 3.7 GB | 251 s | 0.3 ms |
| bodies, `prefix = '1 2'` (as built) | 2.2 GB | 142 s | 21 ms (`the inv`) |
| bodies, no `prefix` | 1.3 GB | 90 s | 2.9 s (`a`) |

Real mail has longer words and a wider vocabulary, so expect 1–1.5 times
these sizes. Keeping the text beside the index would add 0.65 GB here.

## Considered options

- **The server's search (`UID SEARCH`, Gmail's `X-GM-RAW`).** No download,
  but a round trip per Query, no fuzzy ranking, nothing offline, and it
  would only matter until the download ends.
- **The text kept in the Cache.** An entry could then be rebuilt without a
  fetch, at about 1.5 GB more on a large real mailbox.
- **`prefix = '1 2 3 4'` as before.** A millisecond faster on short words,
  for 1.5 GB more.
- **Bodies only as the Preview fetches them.** Free, but a Search would
  depend on what was once opened.
