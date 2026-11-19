# AGENTS.md

Postal is a command-line mail client written in Rust.

## Language rule (mandatory)

**Everything written into this repository is in English**: code comments, commit
messages, log and error messages, documentation, test names, ticket text. Only
the conversation with the user follows the user's language.

## Commit convention (mandatory)

Commits follow [Conventional Commits](https://www.conventionalcommits.org):

```
<type>(<optional scope>): <subject>

<optional body explaining why, wrapped at 72 columns>
```

Types in use: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`. Scope
is an area (`imap`, `smtp`, `cli`), or `deps` for a dependency update, which
is always `chore(deps)`.

- Subject in the imperative mood, lowercase, no trailing period, ≤72 characters
  ("add folder listing", not "Added folder listing.").
- The body explains why; the diff already says what.
- One logical change per commit.
- Commits carry no co-author trailer or attribution line.

## Branches (mandatory)

**Commit and push straight to `main`**, with no branches or pull requests, while
the project is young. Keep each commit to one thing: a refactor, a feature and a
formatting sweep are three commits.

## Type rule (mandatory)

**A struct field exists only if code reads it.** Protocol payloads carry far more
than Postal needs, and every unread field claims a dependency nothing exercises.
Add a field in the change that reads it, and delete it with its last reader. The
same holds for methods, constants and enum variants.

## File rule (mandatory)

**One Rust file holds one concept: a type with its impls, or one piece of
behaviour.** Split by meaning, and keep the public path stable with `pub use` in
the parent. A trait may share a file with its one production implementation, and
private structs mapping a single document may share one file. No linter checks
this.

## Test rule (mandatory)

**Tests are integration tests through an external boundary.** Only mail servers
are faked, at the protocol level, and the whole behaviour is checked. Cover an
edge case with a short synthetic scenario through the same harness. A unit test
is allowed only for a function with many invariants that is far cheaper to
cover directly; it says so in a comment. See `docs/adr/0002-test-boundary.md`.

## Lint rules (mandatory)

- **Clippy runs `all`, `pedantic` and `nursery` as errors**; complexity
  thresholds live in `clippy.toml`.
- **The C++ in `cpp/` is formatted by `clang-format`** (`.clang-format`):
  `clang-format -i cpp/*` before the commit. `vendor/` keeps upstream's style.
- **`clang-tidy cpp/*.cpp` passes** with `bugprone`, `performance` and
  `modernize` as errors (`.clang-tidy`, flags in `compile_flags.txt`).
- **`scripts/sanitize.sh` passes after a change to `cpp/` or `vendor/`**: it
  lays out every cached HTML mail under ASan and UBSan.
- **A review finding a linter could have caught becomes a rule.** Where a lint
  checks it, turn it on in the same change and fix every place it flags.
- A linter exception states its reason: `#[allow(..., reason = "...")]`.

## Comment rule (mandatory)

**A comment is at most three lines**, in every language in the repository, and
it says what is surprising rather than what is visible. A decision that needs a
paragraph goes in `docs/adr/`, with one line in the code pointing at it.

Before writing or editing a comment, read
[.claude/rules/comments.md](.claude/rules/comments.md).

## Spacing rule (mandatory)

**A statement that spans several lines and ends in a brace (`if`, `match`,
`for`, `let ... else`, a struct literal) is followed by a blank line** before
the next statement. rustfmt keeps blank lines but never adds them, and no lint
checks this.

## Working here

- Search the repository through the `fff` MCP (`.mcp.json`), for file names and
  contents alike. It needs `fff-mcp` on `PATH`.
- Non-trivial work starts from an issue, with scope settled there. Small fixes
  go straight to a commit: a typo, an obvious bug, a dependency bump.
- The commit that finishes an issue closes it: `Closes #N` in the body, and
  the push to `main` closes the issue. A commit for part of one says `Refs #N`.
- Tests land with the behaviour they cover.
- While working, run the single test file you touch. Run the full suite once,
  as the last step before the commit: after the code review and its fixes.
- The code is the source of truth, then the docs, then a comment. A doc that
  contradicts the code is a bug in the doc: fix it in the change that found it.

## Agent skills

### Issue tracker

Issues live in GitHub Issues (kryptamine/postal), via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.
