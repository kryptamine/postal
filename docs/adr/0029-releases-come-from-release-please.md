# Releases come from release-please

A user installs Postal without Rust or a C++ compiler (issue #108). Each
release ships prebuilt binaries, and `https://postalmail.dev/install.sh`
fetches the one for the machine.

**Versions.** release-please reads the Conventional Commits on `main` and
keeps a release PR open that bumps `Cargo.toml` and writes `CHANGELOG.md`.
Merging it tags `vX.Y.Z` and creates the GitHub release as a draft; the
same run of `.github/workflows/release.yml` builds and uploads the binaries,
then publishes it. Until then `latest` still names the previous release, so
`postal update` and the install script never meet a release with no
archives; a failed run leaves the draft, and re-running its failed jobs
finishes it. Before 1.0
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
Linux builds on Ubuntu 22.04, so a binary needs its glibc (2.35); an older
or a musl system builds from source; a musl build was passed over, since
2.35 covers every supported Ubuntu and Debian and musl's allocator is slow.
The release links libstdc++, which litehtml needs, statically, so no matching
compiler runtime is needed; a build from source keeps the system's shared
one, since not every distribution ships `libstdc++.a`. The keychain backend
on Linux is zbus, pure Rust, with no libdbus, so neither choice touches it.

**Archive.** `make dist` packs `postal-<target>.tar.gz`: `postal` and the
licenses, next to a `.sha256` file the install script checks. The Makefile
owns the layout; the workflow only uploads it. The script puts the binary in
`~/.local/bin`. The Neovim plugin is not in the archive: a plugin manager
installs it from the repository, on the same release tag.

**Signing.** The macOS binaries are signed with a Developer ID and
notarized, by `scripts/sign-macos.sh` through `make dist SIGN=...`. The
keychain grants an item to the signature's designated requirement, the team
and the identifier `dev.postalmail.postal`; an unsigned binary asked
"Allow access" again after every update. The identifier never changes, or
every user allows access once more. `notarytool` takes a zip of the binary;
a bare binary cannot be stapled, so Gatekeeper fetches the ticket online.
The certificate, its password and an App Store Connect API key (role
Developer) are repository secrets.

**Check.** Before anything is uploaded, each archive is unpacked on a runner
that did not build it and runs `postal --version`: a Linux one in an Ubuntu
22.04 container, checked to load no libstdc++, a macOS one with Gatekeeper
assessing the binary. Only when all four pass are they attached to the
release. This runs only for a release, as macOS runners are scarce and
costly: a failure is fixed and the run rerun, and the assets land on the
same tag.
