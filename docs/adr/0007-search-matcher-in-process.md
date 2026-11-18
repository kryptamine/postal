# Search matches in process with nucleo, not through fzf

Search fuzzy-matches the Cache's sender, subject and Snippet with the `nucleo`
crate inside Postal, instead of suspending the TUI and piping rows through an
`fzf` binary as `$PAGER` is run. The Message List narrows in place, so the
Preview follows the selection while the Query is typed, matched characters are
highlighted in Postal's Theme, and Messages arriving over IDLE or by Backfill
join the results live. None of that is possible behind a suspended screen.

Postal also takes on no runtime dependency it would have to detect and explain
when missing. The cost is that the user's own fzf setup (bindings, extended
syntax, colours) does not apply: Search is plain fuzzy with smart case.

Typing a Query is not an allocation hot path (ADR 0001): matching and the
highlight indices allocate, and the keypress-to-frame budget still holds.

## Considered options

- **External `fzf`.** Least code, and the matcher users already know, but a
  modal hand-off with no Preview, no live rows and no Theme.
- **`fuzzy-matcher` / `skim`'s matcher.** Matches one string at a time on the
  calling thread; `nucleo` scores incrementally on its own workers, which keeps
  a 10,000-Message Cache inside the frame budget.
