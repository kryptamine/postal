# postal.nvim

_Write mail for [Postal](https://postalmail.dev) in Neovim: recipients completed as you type, and an agent beside the Draft._

Postal opens a Draft, a Signature or a Message in your editor. The plugin
completes `To:` and `Cc:` from the people your accounts wrote to or heard
from, completes `Attach:` paths, and `:PostalAgent` opens a chat with an
[ACP](https://agentclientprotocol.com) agent that edits the text for you to
review hunk by hunk.

Needs Neovim 0.10 or newer and `postal` on `PATH`. Recipients come from what
Postal has synced: run `postal` once before.

## Installation

The plugin is the repository itself, its Lua in [`lua/`](../lua/): install
it with lazy.nvim or `vim.pack` as the [README](../README.md#neovim) shows,
on the release tag of the `postal` you run. From a clone, point lazy.nvim's
`dir` at it instead: `{ dir = "~/src/postal", main = "postal", opts = {} }`.

## Usage

| in                 | does                                                         |
| ------------------ | ------------------------------------------------------------ |
| a Draft            | completes `To:`, `Cc:` and `Attach:`; `:PostalAgent` edits the body |
| an `Attach:` line  | shows the file's size, or that it is missing; `gf` edits the file    |
| a Signature        | `:PostalAgent` edits the signature                           |
| a Message (`E`)    | read-only; `:PostalAgent` answers questions about it         |

Completion goes through [blink.cmp](https://github.com/saghen/blink.cmp)
when installed, Neovim's own menu otherwise.

In the chat pane, send with `<CR>` or `<C-s>`; `q` stops the agent's turn,
`<C-c>` ends the chat, `?` lists the keys. The agent reaches only the text,
through an MCP server the plugin hands it: no headers, no commands, no
sending. Its edit lands in the buffer for review:

| key         | does                             |
| ----------- | -------------------------------- |
| `<leader>a` | accept the hunk under the cursor |
| `<leader>r` | reject the hunk under the cursor |
| `<leader>A` | accept every hunk                |
| `<leader>R` | reject every hunk                |

## Configuration

```lua
require("postal").setup({
  cmd = "postal",
  agents = {
    claude = { cmd = { "claude-agent-acp" }, npx = "@agentclientprotocol/claude-agent-acp" },
    gemini = { cmd = { "gemini", "--experimental-acp" } },
  },
  agent = "claude",
  nerd_font = true,
  keys = {},
})
```

| option      | is                                                                          |
| ----------- | --------------------------------------------------------------------------- |
| `cmd`       | the `postal` to run, such as `target/release/postal`                        |
| `agents`    | ACP agents by name; `npx` runs the package when `cmd` is not on `PATH`      |
| `agent`     | the one `:PostalAgent` starts; `:PostalAgent gemini` picks another          |
| `nerd_font` | Nerd Font icons in the pane; on when `vim.g.have_nerd_font` is set          |
| `keys`      | a key by name to another `lhs`, or to `false` to leave it unmapped          |

| key           | default     |
| ------------- | ----------- |
| `send`        | `<CR>`      |
| `send_insert` | `<C-s>`     |
| `stop`        | `q`         |
| `close`       | `<C-c>`     |
| `help`        | `?`         |
| `accept`      | `<leader>a` |
| `reject`      | `<leader>r` |
| `accept_all`  | `<leader>A` |
| `reject_all`  | `<leader>R` |
| `attach`      | `gf`        |

For example, `keys = { accept = "<leader>y", reject_all = false }`.

`claude` is set up by default. The agent signs in through its own CLI
(`claude /login`). `:checkhealth postal` says what is missing.

## Development

`make check` formats, lints and runs the headless smoke check against stub
`postal` and ACP agents.

## License

[MIT](../LICENSE)
