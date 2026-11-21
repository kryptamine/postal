# postal.nvim

Completes the recipients of a Postal Draft in `To:` and `Cc:` from the
addresses your accounts have mail from or wrote to, best first, as
`postal correspondent list` gives them. It works through
[blink.cmp](https://github.com/saghen/blink.cmp) when installed, and with
`<C-x><C-o>` without it.

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
