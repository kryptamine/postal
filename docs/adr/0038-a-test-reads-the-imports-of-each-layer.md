# A test reads the imports of each layer

ARCHITECTURE.md sets the direction layers import in and which crates each
may name: `rusqlite` in `cache` alone, ratatui and crossterm in `ui` alone,
no I/O in `domain`. Module privacy keeps out only what a layer declares
private, so an import against the direction compiled, and only review
caught it. `tests/architecture.rs` reads each file of `src/` and fails on
such an import.

It parses the files with `syn` and checks the paths they name, in `use` and
in code, cut to two segments: `crate::ui`, `std::fs`, `rusqlite::Connection`.
It reads no comment or layout, so a rule of style stays verbal. The rules
sit in two tables beside the test: the layers in order, and the paths one
layer only, or every layer but one, may name. A change to the Direction or
the Invariants of ARCHITECTURE.md changes these tables in the same commit.

It is the one test that reads the source rather than run the binary
(ADR 0002). It does not see the tokens inside a macro call, a path through
`super::` or a module moved by `#[path]`; the repository names other layers
in full, `crate::<layer>::<module>`, so these stay to review. A rule
finer than a layer, such as `render`'s one process or the DNS that only
`mail::discover` speaks, stays to review as well.

## Considered options

- **Keep the rules verbal.** What the repository did until now; a broken
  direction showed up only when someone read the import.
- **One crate per layer in a workspace.** The compiler would hold the
  direction, but every private item a neighbour reads would turn public,
  and the build would split for the sake of one check.
- **An external tool** (ast-grep, Dylint, cargo-modules). One more program
  for every contributor and agent to install, for what one test file with
  `syn` does.
- **Matching the text of `use` lines.** Misses paths named in code and
  grouped imports; `syn` reads both.
