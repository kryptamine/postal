<div align="center">
  <img src="assets/brand/social-preview.png" alt="Postal" width="100%">
  <p>A mail client for the terminal. Works with Gmail, iCloud and any IMAP server, shows HTML mail with its layout, and keeps everything on your computer.</p>
  <p>
    <a href="https://github.com/kryptamine/postal/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/kryptamine/postal/ci.yml?branch=main&logo=githubactions&logoColor=white&label=CI&style=for-the-badge&labelColor=000000" alt="CI status"></a>
    <a href="https://github.com/kryptamine/postal/releases"><img src="https://img.shields.io/github/v/release/kryptamine/postal?logo=github&logoColor=white&color=0797ff&style=for-the-badge&labelColor=000000" alt="Latest release"></a>
    <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.89%2B-0797ff?logo=rust&logoColor=white&style=for-the-badge&labelColor=000000" alt="Rust 1.89+"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-0797ff?style=for-the-badge&labelColor=000000" alt="MIT licence"></a>
  </p>
  <p>
    <a href="https://postalmail.dev"><b>Website</b></a> ·
    <a href="nvim/"><b>Neovim plugin</b></a>
  </p>
</div>

<img src="landing/src/assets/media/hero.png" alt="Postal showing a mailbox and a message" width="100%">

## Install

On macOS 13+ or Linux with glibc 2.35+ (Ubuntu 22.04, Debian 12), arm64 or
x86_64:

```sh
curl -fsSL https://postalmail.dev/install.sh | sh
postal
```

The script puts `postal` in `~/.local/bin`. On first run Postal asks for a
mail provider and a sign-in; `?` lists every key. `postal update` replaces it
with the latest release.

To build from source instead, with Rust 1.89+ and a C++ compiler:

```sh
git clone https://github.com/kryptamine/postal && cd postal
make install
```

`make install` puts `postal` in `~/.cargo/bin`.

## Neovim

Postal opens a Draft in `$EDITOR`. In Neovim 0.10+, the
[plugin](nvim/) completes `To:`, `Cc:` and `Attach:` as you type, and
`:PostalAgent` lets an [ACP](https://agentclientprotocol.com) agent edit the
Draft for you to review hunk by hunk. The plugin is optional, and calls
`postal`: keep it on the latest release with `postal update`.

### [lazy.nvim](https://github.com/folke/lazy.nvim)

```lua
{ "kryptamine/postal", version = "*", main = "postal", opts = {} }
```

### Without a plugin manager

```lua
vim.pack.add({ { src = "https://github.com/kryptamine/postal", version = vim.version.range("*") } })
require("postal").setup({})
```

`vim.pack` needs Neovim 0.12; on older versions clone the repository at its
latest tag into a `pack/*/start/` directory on your `packpath`, then call
`require("postal").setup({})`.

[`nvim/`](nvim/) lists the options, commands and keys.

## Development

```sh
make        # list the targets
make check  # lint, cargo deny, tests and the Neovim check
```
