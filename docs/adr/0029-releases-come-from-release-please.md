# Releases come from release-please

A user installs Postal without Rust or a C++ compiler (issue #108). Each
release ships prebuilt binaries, and `https://postalmail.dev/install.sh`
fetches the one for the machine.

**Versions.** release-please reads the Conventional Commits on `main` and
keeps a release PR open that bumps `Cargo.toml` and writes `CHANGELOG.md`.
Merging it tags `vX.Y.Z` and creates the GitHub release; the same run of
`.github/workflows/release.yml` builds and uploads the binaries. Before 1.0
a `feat` bumps the minor version and a breaking change does too. The
manifest starts at `0.0.0`, below `Cargo.toml`'s `0.1.0`, so the first
`feat` releases `0.1.0`; `bootstrap-sha` starts the changelog at the commit
before release-please, not at the first commit of the repository.

**Token.** A PR opened with `GITHUB_TOKEN` starts no workflow, so the `ci`
check the ruleset requires would never run on the release PR. The workflow
requires `RELEASE_PLEASE_TOKEN`, a fine-grained token with contents and
pull request write access on this repository, and fails without it rather
than open a PR that can never merge.

**Targets.** `aarch64` and `x86_64`, macOS and Linux. Both macOS builds run
on an arm64 runner, the Intel one cross-compiled, for macOS 13 and later.
Linux builds on Ubuntu 22.04, so a binary needs its glibc (2.35) and
libstdc++, which litehtml links; an older or a musl system builds from
source. The keychain backend on Linux is zbus, pure Rust, with no libdbus.

**Archive.** `make dist` packs `postal-<target>.tar.gz`: `postal` and the
licenses, next to a `.sha256` file the install script checks. The Makefile
owns the layout; the workflow only uploads it. The script puts the binary in
`~/.local/bin`. The Neovim plugin is not in the archive: a plugin manager
installs it from the repository, on the same release tag.
