# postal.nvim

Completes the recipients of a Postal Draft in `To:` and `Cc:` from the
addresses your accounts have mail from or wrote to, best first, as
`postal correspondent list` gives them. It works through
[blink.cmp](https://github.com/saghen/blink.cmp) when installed; without
it, Neovim's own menu opens as you type (`'autocomplete'`, Neovim 0.12),
and `<C-x><C-o>` asks for it on an older one.

A Draft's headers are `To:`, `Cc:`, `Subject:` and `Attach:`, one file
per `Attach:` line and as many lines as files:
`Attach: ~/Documents/report.pdf`. A relative path is under your home
directory, and the plugin completes it as you type.

Needs Neovim 0.10 or newer, and `postal` on `PATH`.

With [lazy.nvim](https://github.com/folke/lazy.nvim):

```lua
{
  "kryptamine/postal",
  config = function(plugin)
    vim.opt.rtp:append(plugin.dir .. "/nvim")
    require("postal").setup()
  end,
}
```

While working on Postal itself, `{ dir = "~/Work/postal/nvim", config = ... }`
picks up changes at once. `make install` at the repository's root
instead installs the checkout as a user runs it: a release `postal` in `~/.cargo/bin`, and a
copy of the plugin in `~/.local/share/postal/nvim` for `dir` to point at. `setup({ cmd = "/path/to/postal" })` runs a
`postal` that is not on `PATH`, such as `target/release/postal`.

The list is what Postal has synced: run `postal` once before.

## Writing with an agent

`:PostalAgent` in a Draft opens a chat pane beside it with an agent that
speaks [ACP](https://agentclientprotocol.com), the cursor in its input.
Each message sits under `## Me` or `## <agent>`; type under the last
`## Me` and send with `<CR>` or `<C-s>`. `q` stops the agent's turn,
`<C-c>` ends the chat, `?` lists the keys. A spinner shows while the agent
works, a dim line each tool call it was denied or the edit it made, and
the winbar the keys and the hunks left to review. The agent reads the
Thread the Draft answers, from `postal thread show`, and the body as it
stands each turn, your own edits included. It reaches the body only
through the `read_draft` and `write_draft` tools of an MCP server the
plugin hands it, so the agent must take MCP servers from its client. It
cannot see or change the headers, run commands, or send. The Draft's
text goes to whichever provider the agent talks to, and only once you
ask.

The agent's edit lands in the buffer, never on disk, for review hunk by
hunk: its lines highlighted, the ones they replace shown over them.
`<leader>a` accepts the hunk under the cursor and `<leader>r`
rejects it; `<leader>A` and `<leader>R` take or reject every
hunk left, and the pane's top line counts them. A hunk over lines you
changed while the agent worked is a conflict: your lines stay until you
accept the agent's, for the whole hunk. The next instruction waits for
the review to end.

```lua
require("postal").setup({
  agents = {
    claude = { cmd = { "claude-agent-acp" }, npx = "@agentclientprotocol/claude-agent-acp" },
    gemini = { cmd = { "gemini", "--experimental-acp" } },
  },
  agent = "claude",
  nerd_font = true,
})
```

`claude` is set up by default. When `cmd` is not on `PATH`, the `npx`
package runs through `npx` instead, so Node.js is all it needs; the first
start fetches it. `:checkhealth postal` says what is missing and how to
get it. `:PostalAgent gemini` starts another agent by name. The agent
signs in through its own CLI (`claude /login`); when it asks for that,
the pane says how. `nerd_font` draws the pane's icons from a Nerd Font,
on by default when `vim.g.have_nerd_font` is set. A session lasts as
long as the Draft's buffer.

## Reading with an agent

`E` in Postal opens the selected Message in your editor, read-only: its
headers, then its body as text, its Quote kept and each link's URL listed
at the end, for searching and copying. Postal removes the file once the
editor exits. `:PostalAgent` in it opens the same chat pane, to ask about
the Message: the agent gets it whole in the first prompt, and through the
`read_message` tool, and can change nothing. The keys are a Draft's, with
no review.

`make check` formats, lints and runs the headless smoke check, against
stub `postal` and ACP agents, the gate before a commit; `make` lists every target.
