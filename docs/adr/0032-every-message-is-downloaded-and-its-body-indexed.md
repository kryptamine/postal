# Every Message is downloaded, and its body indexed

A Search matched only what the Cache held of a Message: sender, subject,
Snippet and recipients (ADR 0023). A word past the Snippet found nothing,
though the server had it. Desktop clients that search everything, notmuch,
Thunderbird, Apple Mail, download the mailbox and index it themselves;
Postal does the same, so a Search stays in process (ADR 0007), matches
bodies, and works offline. Issue #157.

## Three passes in turn

Once the pass over Sent is done, the fetch connection's `SpareWork` runs
three long passes, a step each in turn, so none waits on another to end:
every Folder whole into the Cache, the bodies the index lacks, and the
envelopes the Correspondents are learnt from (ADR 0031).

## The Folder pass

- On its first step it lists the Folders. On Gmail it walks All Mail
  alone, which holds every label's Messages once, and leaves out Trash and
  Spam as a Search does (ADR 0010); elsewhere it walks every Folder but
  Trash and Spam.
- A Folder never synced gets its newest 500 Messages as the first sync
  does, `EXAMINE`d: this connection never stores a flag.
- Then, a step at a time, the 100 Messages older than every cached one, as
  a Backfill does, until the server has nothing older. A Preview waits on
  one such page at most, or on that first fetch of a Folder.
- A Folder the server refuses to give is passed over.
- It runs once per connection; only the shown Folder is kept fresh after,
  by IDLE (ADR 0008). Moves and expunges in another Folder reach the Cache
  and the index when it is next shown.

## The body pass

- A batch is the 20 newest Messages whose body the index lacks, all in the
  Folder of the newest one. Trash and Spam are indexed too, as far as they
  are cached, so counting what is left reads the partial index alone; a
  Search still reads them only for an `in:`. Their text parts (ADR
  0024) are fetched with `BODY.PEEK[n]<0.32768>`: past 32 KiB a body is
  mostly quoted history. Attachments are never fetched.
- Each batch is one Cache write, in its own transaction, so the sync
  connection's writes never wait long behind it.
- A request (a Preview, a Reply, a Backfill) waits on one batch at most:
  the queue is read after every batch, and requests go first.
- A Message still without a Snippet gets it cut from the same fetch.
- Once no body is left, the pass looks again every 5 seconds, for mail
  IDLE brought, a Backfill or a resync wrote.
- Gmail's `[OVERQUOTA]` rests the pass for a day. A batch refused for
  another reason is indexed without its bodies, or the pass would ask for
  it forever. `[THROTTLED]` is an outage: the connection backs off and
  reconnects, and the passes start again from what the Cache holds.

## Progress

While a Search is open, its hint line shows how far the index is after the
Query, `search index 37% (120,312 of 325,000)`, until it is whole. Indexed
are the cached Messages with their body; the total adds to every cached
Message what the server last counted in each walked Folder past the rows
cached there, so the share does not fall back as the Folder pass brings
rows. A Folder never synced counts once its first fetch counts it.

Counting reads the partial index and every row's Folder once, 22 ms at
500,000 (84 ms if Trash and Spam were left out of the body pass, which is
why they are indexed as far as cached). So it is counted every 5 seconds
at most while a pass runs, and at once when the passes end.

## Ranking a match in a body

FTS5 finds the candidates and nucleo ranks them by the card's text (ADR
0023), and what nucleo did not match was dropped. A Message matched only in
its body has no card text to match, so a Thread nucleo cannot score now
comes after every scored one, newest first. Smart case still holds: a
Thread nucleo matches ignoring case, but not as typed, is dropped. Case is
not known of a body, so `Zeppelin` also finds `zeppelin` there.

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
  `X-GM-MSGID`, and each copy is indexed from one fetch. Elsewhere the key
  is UIDVALIDITY and UID, which two Folders may share, so only the row
  fetched is indexed.
- A row replaced while its batch was on the wire, with another body key,
  is left for the next batch.
- `message_unbodied`, a partial index, finds the next batch without
  reading the indexed rows.

A negated word, `-word`, now leaves out a Message with it in its body too.

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
