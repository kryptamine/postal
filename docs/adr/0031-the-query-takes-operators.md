# The Query takes operators, and a menu completes them

A Query was only words (ADR 0023): no way to ask for a sender, a date,
Unread mail or attachments, and a broad word drowned in the `N` = 500
newest candidates. Now a Query mixes free words with operators, in Gmail's
syntax so it is already familiar, and a menu over the Query line suggests
what the word being typed can become. Issue #156.

## The syntax

| Term | Matches |
| --- | --- |
| a word | the start of a word of the sender, subject, Snippet or, of mail the Account sent, its recipients, as before; its body too since ADR 0032 |
| `from:x` | the From line, name and address |
| `to:x`, `cc:x` | `To` and `Cc` of every Message: the Cache keeps them as one line, so the two are one |
| `subject:x` | the subject |
| `is:unread`, `is:read` | the Unread flag |
| `has:attachment` | one or more attachments |
| `in:x` | the Folder whose path or role label is `x`, in any case; Trash and Spam too |
| `account:x` | the Accounts whose address holds `x` |
| `before:YYYY-MM-DD`, `after:` | arrival before that local midnight, or at it and after; `/` also parts the date |
| `older:7d`, `newer:7d` | arrival over, or within, so many days, weeks, months or years (`d`, `w`, `m`, `y`) |
| `"a b"` | the words adjacent, in order; an operator's value takes quotes too |
| `-term` | negation of any of the above |

- An operator's value of several words, `from:p.novak`, is a phrase: the
  words in order, the last a prefix.
- An unknown operator, `invoice:1042`, is searched as words, name and all.
  A known one with no value or one that does not parse yet, `before:2024-1`
  while it is typed, is no term, so the list does not jump to words.
- `is:flagged` is left out: Postal does not keep `\Flagged`.
- `in:` and `account:` pick the Folders read; more than one of a kind is
  either. Only an `in:` naming them reads Trash and Spam.
- `postal message search` and the MCP `message_search` parse the same
  Query; one starting with `-` is no flag there.

## The SQL

Every term but the Folders becomes part of one statement, built per Query,
so a narrow Query is cut by `N` only after it narrows:

- Words go to FTS5 as quoted prefixes behind a column filter, so no input
  reaches FTS5 or SQL syntax: a bare word `{sender subject snippet
  recipients} : "w"*`, `from:` the `from_line` column, `to:` `to_line`,
  `subject:` `subject`. Negated words are `(held) NOT (shunned)`; with no
  words held, `rowid NOT IN` the rows matching them.
- `in:` and `account:` decide the Folders, bound as `:places`, a JSON array
  of `[account, mailbox]`; the TUI's copy ranking (ADR 0010) is unchanged.
- Flags and attachments are `WHERE` clauses on `message`; dates are on the
  rowid, the arrival second shifted left 24 bits, so FTS5 reads only that
  range.

`to:` on received mail needed recipients in the index, which held them
only for sent mail, and `from:` an address the `sender` column lacks. FTS5
adds no column in place, so schema step 3 builds `message_text` anew with
two more columns, `from_line` and `to_line` of every Message. A bare word
does not reach them, or it would match the Account's own address on
everything received. On disk: see Measured.

## A flag that changes under the Search

A Dwell marks the selected match read. Run again on the `Update` that
follows, `is:unread` would drop it and select the next, which the next
Dwell reads in turn: one Search would read every Unread Message. So a run
for the same Query lets a flag term pass the rows the last run found, bound
as `:kept`; typing anew forgets them.

## The menu

A menu over the Query line, from the column the last word starts at, as a
shell or Neovim completes:

| Word | Suggests |
| --- | --- |
| none, on an empty Query | every operator, with what it matches |
| a bare word | the operators whose name it starts |
| `from:`, `to:`, `cc:` | Correspondents whose address or a word of whose name it starts, those written to first, then by how much mail names them |
| `in:` | Folders whose label holds it |
| `account:` | Accounts whose address starts with it |
| `is:`, `has:` | their values |

It opens on its own while the word has suggestions, and not after a space,
so `/` shows it at once. `Tab` enters it and `Shift-Tab` goes back, writing
each suggestion in place of the word, a value with a space quoted. In the
menu, `j`/`k` and `↓`/`↑` go round it too, and `Enter` takes the
suggestion and leaves it, the menu suggesting anew: after `from:`, the
Correspondents. Any other key leaves the menu and acts as it would. `Esc`
closes it, and only a second `Esc` ends the Search. Out of the menu,
`Enter`, `↑↓` and `^j/^k` keep their meaning. The Correspondents are read once, the
first time a word asks for them. `?` types into the Query, so the empty
Query's menu, not the help popup, is where the operators are listed.

The Query line shows each operator's `name:` and a negation's `-` in the
key colour.

## Measured

`sqlite3` on the reference machine, 500,000 synthetic Messages as in
ADR 0023 (70% Inbox, 20% Archive, 5% Trash, 5% Sent, 10% Unread, 10% with
attachments, 3,000 senders), reading the newest `N` candidates:

| Query | Without the rowid scan | As built |
| --- | --- | --- |
| `a`, `th b`, `from:a`, `to:a`, `subject:th` | | 0.6–1.6 ms |
| `th is:unread`, `has:attachment from:a` | | 5.6–6.9 ms |
| `is:unread` | 275 ms | 0.9 ms |
| `is:unread has:attachment` | 275 ms | 12 ms |
| `in:Trash` | 58 ms | 2.0 ms |
| `before:2018-01-01` | 40 ms | 0.3 ms |
| `-the` | 285 ms | 2.2 ms |

A Query of no words held reads `message` by rowid, `NOT INDEXED`: by the
primary key the planner sorted every row of the Folders read. The cost is
a filter matching under `N` Messages of all, which scans the whole table:
100 ms at 500,000. Ranking and the window are as ADR 0023 measured them.

The two columns grow the index from 512 MB to 640 MB on that Cache, a
quarter more: every received Message names the Account in `to_line`, and
`from_line` repeats the sender's name beside the address.

The Correspondents, 1.5 million `correspondence` rows of 3,000 addresses,
take 0.8 s to read and rank, so a thread starts on it at `/`; the menu
waits for it only if `from:` comes first.

## Considered options

- **A modal, as Slack's.** Room for sections and recent Queries, but it
  hides the list, so a result shows only after `Enter`, and it is a second
  way to search beside `/`.
- **Tokens as chips.** An accepted operator drawn as `[from: Novák ×]`,
  erased whole. Editing inside the Query gets harder; it can come on top.
- **Highlighting alone.** Cheapest, but nothing tells the operators or the
  Correspondents' addresses apart from typing them out.
- **`to:` over `recipients` only.** No new index, but `to:` would find only
  mail the Account sent, which no one expects of it.
