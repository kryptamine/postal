# Contributing

Thanks for wanting to help. Postal is young and has one maintainer, so a short
conversation first saves both of us work.

## Before you write code

Open an issue for anything beyond a typo or an obvious bug, and settle the
scope there. A pull request without one may be closed and turned into an
issue.

Security problems go to [SECURITY.md](SECURITY.md), never to a public issue.

## Building

You need Rust 1.89+ and a C++17 compiler; Neovim for the plugin's checks.

```sh
make          # list the targets
make check    # lint, cargo deny, tests and the Neovim check
make install  # a release build where a user has it
```

`make check` needs `cargo-nextest`, `cargo-deny`, `clang-format`,
`clang-tidy`, `stylua` and `nvim` on `PATH`. A change to `landing/` also
needs `make landing` (Node and pnpm).

`make coverage` (`cargo-llvm-cov`) shows the lines no test runs, and
`make mutants FILE=src/...` (`cargo-mutants`) the changes to a file no test
notices, and `make mutants-diff` those among the lines a branch changed, a
run of minutes before a pull request; none is part of `make check`.

## The rules

[AGENTS.md](AGENTS.md) holds the rules every change follows, for people and
coding agents alike. The ones a review checks first:

- **Tests go through an external boundary**: the mail servers are faked at
  the protocol level, and the whole behaviour is checked. See
  [ADR 0002](docs/adr/0002-test-boundary.md).
- **One file holds one concept**, and a struct field exists only if code reads
  it.
- **A comment is at most three lines.** A longer decision goes in
  [`docs/adr/`](docs/adr/).
- [ARCHITECTURE.md](ARCHITECTURE.md) says which layer a new module belongs in.

## Commits and pull requests

Commits follow [Conventional Commits](https://www.conventionalcommits.org):
`feat(imap): add folder listing`, imperative and lowercase, with a body that
says why. One logical change per commit; a refactor and a feature are two.

Every change reaches `main` through a pull request that passes CI. Put
`Closes #N` in the commit that finishes an issue.

## License

By contributing you agree that your work is licensed under the
[MIT License](LICENSE).
