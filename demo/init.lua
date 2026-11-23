-- The Neovim of a demo run: the plugin of this checkout, the demo's scripted
-- agent, and the terminal's own palette rather than Neovim's colours.
vim.opt.rtp:prepend("@PLUGIN@")
vim.o.termguicolors = false
vim.o.number = false
vim.o.laststatus = 0
vim.o.ruler = false
vim.o.showmode = false
vim.o.shortmess = vim.o.shortmess .. "I"
vim.g.have_nerd_font = true

require("postal").setup({
	cmd = "@POSTAL@",
	agents = { demo = { cmd = { "nvim", "--clean", "-l", "@AGENT@" } } },
	agent = "demo",
})
