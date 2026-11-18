# Search matches in process with nucleo, not through fzf

Search fuzzy-matches the Cache's sender, subject and Snippet with
`nucleo-matcher`, the matcher of the `nucleo` project, inside Postal, instead
of suspending the TUI and piping rows through an `fzf` binary as `$PAGER` is
run. The Message List narrows in place, so the
Preview follows the selection while the Query is typed, matched characters are
highlighted in Postal's Theme, and Messages arriving over IDLE or by Backfill
join the results live. None of that is possible behind a suspended screen.

Postal also takes on no runtime dependency it would have to detect and explain
when missing. The cost is that the user's own fzf setup (bindings, extended
syntax, colours) does not apply: Search is plain fuzzy with smart case.

Typing a Query is not an allocation hot path (ADR 0001): matching
allocates, and the keypress-to-frame budget still holds. Each keypress scores
every candidate again, spread over a scoped thread per core: on 10,000
Messages that is at most 3 ms on the reference machine, against 17 ms on one
thread. A Query that only grows rescores the previous matches alone.
Highlights are computed for the visible cards at draw time, into buffers kept
between frames, so moving through the matches does not allocate.

## Considered options

- **External `fzf`.** Least code, and the matcher users already know, but a
  modal hand-off with no Preview, no live rows and no Theme.
- **`fuzzy-matcher` / `skim`'s matcher.** Slower per string than
  `nucleo-matcher`, which follows fzf's scoring.
- **`nucleo`'s `Nucleo` worker pool.** Scores incrementally in the
  background, but its pattern always parses fzf's extended syntax, and its
  results arrive on a later tick, after the keypress's frame.
