# Performance budget and allocation policy

Postal is meant to feel instant, so the MVP holds measurable targets and a
narrow allocation policy instead of a general "be fast". Timings are checked by
hand on the reference machine, because shared CI runners are too noisy; the
allocation count is deterministic, so it is asserted in CI.

## Budget

Reference: the maintainer's Mac, with a Cache of 10,000 Messages (about 20
times the first-run fetch).

| Target | Budget | Check |
| --- | --- | --- |
| Cold start to a Message List drawn from Cache | ≤ 100 ms | `hyperfine` |
| Keypress to frame (scroll, change selection) | ≤ 16 ms | manual trace |
| Steady-state RSS after scrolling the whole list | ≤ 30 MB | `/usr/bin/time` |
| `EXISTS` received to new row on screen | ≤ 500 ms | manual trace |
| Allocations per frame in steady state | 0 | CI, `alloc-count` |

- Cold start reads only the visible window of rows from Cache, never the whole
  Inbox. The network is not part of it.
- A Preview not yet rendered shows a placeholder; it never blocks a frame.
- Rendered Previews live in a byte-bounded LRU of about 8 MB. The converter's
  transient peak (up to ~14 MB on a 395 KB outlier) is outside the RSS budget
  but must be released. The rendered View's engine runs in a process of its
  own, so its memory is the system's again once the layout is back (ADR 0006).
- Cold start includes asking the terminal for its background: one round trip,
  or 100 ms where a terminal answers neither query.
- IDLE latency is measured from the client receiving `EXISTS`; Gmail's own
  delay is not ours to control.

A red allocation test blocks a release. A timing or RSS regression over 20%
needs an issue that explains it.

## Allocation policy

Hot paths do not allocate in steady state: borrow (`Cow`, `&str` into a read
buffer) and reuse buffers. The hot paths are:

1. the frame loop (layout and drawing);
2. scrolling and selection in the Message List, including reading the row
   window from Cache;
3. parsing IMAP responses (`FETCH`, IDLE);
4. showing an already rendered Preview.

An enabled `tracing` event allocates, so hot paths emit nothing at `info` or
above; debug and trace events there are allowed, because the allocation check
runs at the default level.

Everywhere else (Onboarding, config, sync logic, the HTML converter, which runs
off the UI thread and is cached) `clone` is fine when it reads more simply.
Arenas only after a measurement shows the need.

## Check

A test-only Cargo feature `alloc-count` swaps in a counting global allocator;
the binary logs allocations per frame. A pty test scrolls the Message List
through the fake IMAP server and asserts zero allocations per frame. Release
builds keep the system allocator.

The count covers the UI thread only, from the input or Cache read that starts
a frame to the end of its draw; waiting for input and the log writer's thread
are left out. CI runs it as
`cargo test --features alloc-count --test frame_allocations`.
