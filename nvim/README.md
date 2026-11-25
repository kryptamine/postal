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

| in                | does                                                                      |
| ----------------- | ------------------------------------------------------------------------- |
| a Draft           | completes `To:`, `Cc:` and `Attach:`; `:PostalAgent` edits the body       |
| a Draft's body    | warns when it mentions an attachment and no `Attach:` names a file        |
| a Markdown Draft  | parses what you write as Markdown: render-markdown.nvim renders it        |
| `To:` and `Cc:`   | marks a recipient a send would refuse, and an address no mail holds       |
| `:PostalContacts` | picks a Correspondent and adds it to `To:`, or the `Cc:` under the cursor |
| `:PostalSend`     | writes and sends the Draft, unless a check warns; `!` sends anyway        |
| an `Attach:` line | shows the file's size, or that it is missing; `gf` edits the file         |
| a Message's `[1]` | `gx` opens the link; `:PostalLinks` lists every link                      |
| a Reply's Quote   | folds away closed: `zo` opens it                                          |
| a Signature       | `:PostalAgent` edits the signature                                        |
| a Message (`E`)   | read-only; `:PostalAgent` answers questions about it and its Thread       |

`:PostalSend` exists in a Draft the TUI opened, which waits for the
editor's exit: Postal passes the exit status that sends the Draft in
`$POSTAL_SEND_STATUS`. An agent's edit waiting for review holds it back,
and any other exit keeps the Draft in Local Drafts. Map
it as you like: `vim.keymap.set("n", "<leader>s", "<cmd>PostalSend<cr>")`.

A Draft is Markdown when its Account sets `compose-format = "markdown"`,
which Postal tells the editor in `$POSTAL_COMPOSE_FORMAT`. Its filetype is
then `mail.markdown`, and Neovim's own Markdown parser reads the body up to
the signature, a Forward's original or the Quote, which Postal sends as
written. [render-markdown.nvim](https://github.com/MeanderingProgrammer/render-markdown.nvim)
renders it when installed, with no setting; without it, the Markdown is
highlighted and its marks hidden off the cursor's line.

Completion goes through [blink.cmp](https://github.com/saghen/blink.cmp)
when installed, Neovim's own menu otherwise.

`:PostalAgent shorter` opens the chat and sends a prompt set up by name:
`proofread`, `shorter` and `formal` are there by default over a Draft or a
Signature, `summary` over a Message, and `:PostalAgent gemini shorter`
names the agent too.

In the chat pane, send with `<CR>` or `<C-s>`; `q` stops the agent's turn,
`<C-c>` ends the chat, `<C-p>` and `<C-n>` bring back what you sent in any
chat, `?` lists the keys. The agent reaches only the text, through an MCP
server the plugin hands it: no headers, no commands, no
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
  attachment_words = { "attach", "enclos" },
  spell = false,
  prompts = {},
})
```

| option             | is                                                                      |
| ------------------ | ----------------------------------------------------------------------- |
| `cmd`              | the `postal` to run, such as `target/release/postal`                    |
| `agents`           | ACP agents by name; `npx` runs the package when `cmd` is not on `PATH`  |
| `agent`            | the one `:PostalAgent` starts; `:PostalAgent gemini` picks another      |
| `nerd_font`        | Nerd Font icons in the pane; on when `vim.g.have_nerd_font` is set      |
| `keys`             | a key by name to another `lhs`, or to `false` to leave it unmapped      |
| `prompts`          | instructions by name for `:PostalAgent <name>`; `false` drops a default |
| `spell`            | spell checks a Draft and a Signature, in your own `spelllang`           |
| `attachment_words` | how each word mentioning an attachment starts, case aside               |

| key           | default     |
| ------------- | ----------- |
| `send`        | `<CR>`      |
| `send_insert` | `<C-s>`     |
| `stop`        | `q`         |
| `close`       | `<C-c>`     |
| `help`        | `?`         |
| `previous`    | `<C-p>`     |
| `next`        | `<C-n>`     |
| `accept`      | `<leader>a` |
| `reject`      | `<leader>r` |
| `accept_all`  | `<leader>A` |
| `reject_all`  | `<leader>R` |
| `attach`      | `gf`        |
| `link`        | `gx`        |

For example, `keys = { accept = "<leader>y", reject_all = false }`.

`claude` is set up by default. The agent signs in through its own CLI
(`claude /login`). `:checkhealth postal` says what is missing,
and whether `postal` is the release the plugin came with.

## Development

`make check` formats, lints and runs the headless smoke check against stub
`postal` and ACP agents.

## License

[MIT](../LICENSE)
