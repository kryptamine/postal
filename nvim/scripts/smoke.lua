-- Run by `make smoke` and tests/nvim.rs: in a bare nvim, a Draft's `Cc:`
-- completes a recipient from Postal's Correspondents, and its body nothing.
local function fail(msg)
	io.stderr:write("smoke: " .. msg .. "\n")
	vim.cmd("cquit 1")
end

local stub = vim.fn.fnamemodify("scripts/postal-stub", ":p")
require("postal").setup({ cmd = os.getenv("POSTAL_BIN") or stub })

local drafts = vim.fn.tempname() .. "/postal/drafts"
vim.fn.mkdir(drafts, "p")
local draft = drafts .. "/1.eml"
vim.fn.writefile({
	"To: bob@example.com",
	"Cc: Ada Lovelace <ada@example.com>, pet",
	"Subject: Hi",
	"",
	"pet",
}, draft)
vim.cmd.edit(draft)

if vim.bo.omnifunc ~= "v:lua.require'postal'.omnifunc" then
	fail("omnifunc not set, it is " .. vim.inspect(vim.bo.omnifunc))
end

-- Without blink, Neovim's own menu opens as a recipient is typed
if vim.fn.exists("+autocomplete") == 1 and not (vim.bo.autocomplete and vim.bo.complete == "o") then
	fail("autocomplete not fed by the omnifunc alone")
end

local omnifunc = require("postal").omnifunc
local function at(row, col)
	vim.api.nvim_win_set_cursor(0, { row, col })
	local start = omnifunc(1, "")
	if start < 0 then
		return start, {}
	end
	local base = vim.api.nvim_get_current_line():sub(start + 1, col)
	return start, omnifunc(0, base).words
end

-- The recipient after the comma, not the whole header
local before = "Cc: Ada Lovelace <ada@example.com>, "
local start, items = at(2, #before + #"pet")
if start ~= #before then
	fail("recipient starts at " .. start .. ", not " .. #before)
end

local words = vim.tbl_map(function(item)
	return item.word
end, items)
local want = { "Petr Novak <petr@example.com>", "Peter Pan <pan@example.com>" }
if not vim.deep_equal(words, want) then
	fail("offered " .. vim.inspect(words))
end

if at(5, #"pet") ~= -3 then
	fail("the body offered recipients")
end

io.stdout:write("smoke: ok\n")
vim.cmd("qall!")
