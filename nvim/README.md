# postal.nvim

Completes the recipients of a Postal Draft in `To:` and `Cc:` from the
addresses your accounts have mail from or wrote to, best first, as
`postal correspondent list` gives them. It works through
[blink.cmp](https://github.com/saghen/blink.cmp) when installed; without
it, Neovim's own menu opens as you type (`'autocomplete'`, Neovim 0.12),
and `<C-x><C-o>` asks for it on an older one.

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
picks up changes at once. `setup({ cmd = "/path/to/postal" })` runs a
`postal` that is not on `PATH`, such as `target/release/postal`.

The list is what Postal has synced: run `postal` once before.

## Writing with an agent

`:PostalAgent` in a Draft opens a chat pane beside it with an agent that
speaks [ACP](https://agentclientprotocol.com), and asks what to change;
`<CR>` in the pane asks again. The agent reads the Thread the Draft
answers, from `postal thread show`, and the body as it stands each turn,
your own edits included. It cannot see or change the headers, run
commands, or send. The Draft's text goes to whichever provider the agent
talks to, and only once you ask.

The agent's edit lands in the buffer, never on disk, for review hunk by
hunk: its lines highlighted, the ones they replace shown over them.
`<LocalLeader>a` accepts the hunk under the cursor and `<LocalLeader>r`
rejects it; `<LocalLeader>A` and `<LocalLeader>R` take or reject every
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
})
```

`claude` is set up by default. When `cmd` is not on `PATH`, the `npx`
package runs through `npx` instead, so Node.js is all it needs; the first
start fetches it. `:checkhealth postal` says what is missing and how to
get it. `:PostalAgent gemini` starts another agent by name. The agent
signs in through its own CLI (`claude /login`); when it asks for that,
the pane says how. A session lasts as long as the Draft's
buffer.

`make check` formats, lints and runs the headless smoke check, against
stub `postal` and ACP agents, the gate before a commit; `make` lists every target.
