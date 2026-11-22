# Search finds candidates in an FTS5 index and ranks them with nucleo

A Search asks a SQLite FTS5 index of the Cache for the Messages whose
words start with the Query's words, reads the text of at most the newest
`N` of them, and ranks those with nucleo as before. Nothing of the Cache is
held in memory for a Search but those candidates and the Threads they
form. This replaces how ADR 0007 gets its candidates; its matcher, ranking
and highlights stay.

## Why

A Search loaded every cached Folder but Trash and Spam into `Message`
values, with one nucleo haystack each, and loaded it all again on every
`Update::Inbox` while open. Time and memory grew with the whole Cache.
Measured on the reference machine with 500,000 synthetic Messages (about
250 bytes of sender, subject and Snippet each):

| Way | Per keypress | Index on disk | Memory |
| --- | --- | --- | --- |
| nucleo over the whole Cache (ADR 0007, 3 ms per 10,000) | ~150 ms | none | ~150 MB |
| FTS5 `trigram`, substrings | 0.6–11 ms; ~140 ms for 1–2 characters | 704 MB | candidates |
| FTS5 `unicode61`, word prefixes | 0.2–2.3 ms | 337 MB | candidates |

Both budgets of ADR 0001 break at that size with the corpus in memory.
`trigram` keeps matches inside a word, but its index is five times the
text and a Query under three characters falls back to a scan. Word
prefixes are how Gmail, Apple Mail and Thunderbird's Gloda search mail.

## The design

- **The index.** One FTS5 table over the `message` rows, `unicode61` with
  diacritics removed and prefix indexes, contentless with
  `contentless_delete=1`: Postal reads the text from `message`, so FTS5
  keeps none. Its columns are the sender, the subject, the Snippet and, for
  a Message the Account sent, its recipients' names and addresses.
  Triggers on `message` keep it, so `mail` is untouched and an arriving
  Message is findable once its row is written. They also set each row's
  rowid to its arrival second shifted left 24 bits, so the index reads
  newest first and a Query stops after `N` candidates, however many
  match; sorting the matches by arrival cost 260 ms for a one-letter
  Query. Before the first release the schema's first step holds it
  (ADR 0021).
- **The Query.** It is cut into words by the same rules as the text, each
  a quoted prefix term, all required: `p.novak` asks for `"p"* "novak"*`.
- **Candidates.** The matching rows of every Folder but Trash and Spam of
  every Account, each Message once, the shown Folder's copy winning
  (ADR 0010), newest first, at most `N`. `N` is 500, set by measurement
  below; when more match, the hint line says so, to type more. A Query
  with no words, empty or punctuation alone, takes the newest `N` of all.
- **Ranking.** nucleo scores the candidates' haystacks with the Query as
  now: smart case and normalisation, the Thread ranked by its best Message,
  a tie to the newer. A candidate nucleo does not match is dropped, which
  is how an uppercase letter still makes the Query case-sensitive.
- **Showing.** The matched Threads are rows as in ADR 0022, their cards
  read for the window; highlights are computed for the visible cards.
  Only the candidates' text is held, for nucleo. Reading the window again
  allocates nothing once its buffers have grown, as ADR 0001 asks.
- **Updates.** An `Update::Inbox` while a Search is open runs it again.

What is lost against ADR 0007: a fuzzy match across a word's letters
(`jdoe` for John Doe) and a match inside a word (`voice` for invoice).
Within `N`, the best match still comes first; past it, a broad Query shows
the newest candidates' best.

## Measured

Release build, the reference machine, 500,000 Messages seeded in the
Cache: about 250 bytes of sender, subject and Snippet each, words from
`/usr/share/dict/words` at Zipf frequencies, the frequent ones short, and
Threads of three. Each of 68 keypresses typed 150 ms after the last, from
the key to the Search ranked and its window read:

| `N` | Mean | Worst | Over 16 ms | The Search's memory |
| --- | --- | --- | --- | --- |
| 500 | 6.4 ms | 17.1 ms | 2 | 0.2 MB |
| 1,000 | 9.6 ms | 26.5 ms | 13 | 0.3 MB |
| 2,000 | 13.8 ms | 35.6 ms | 24 | 0.6 MB |

Reading the candidates is most of it, and grows with `N`; a CPU idle for
150 ms runs these queries about three times slower than back to back. The
two over 16 ms at 500 are `kathy b`: FTS5 merges both words' lists of
rows, whatever `N`. Holding every candidate's whole Thread instead of the
window cost 15–50 ms and 5–10 MB at 2,000. The Cache took 855 MB.

## Considered options

- **The whole Cache in memory, as before.** Keeps fuzzy matching, but at
  500,000 Messages a keypress costs ~150 ms and the corpus ~150 MB.
- **The same scan on a thread of its own.** Memory stays small if rows are
  streamed, but results arrive several frames after the keypress, which
  ADR 0007 rejected for nucleo's worker pool.
- **FTS5 `trigram`.** Keeps matches inside a word, for an index five times
  the text and a scan under three characters.
- **FTS5 alone, ranked by `bm25`.** Less code, but no smart case, no
  ranking by the best match, and highlights to compute apart anyway.
