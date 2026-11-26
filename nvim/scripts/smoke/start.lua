-- An agent that cannot start, or needs a sign-in, says what to do.
local s = ...
local fail = s.fail
local drafts = s.drafts
local setup = s.setup
local acp_stub = s.acp_stub
local said = s.said
local home = s.home

-- An agent wanting a sign-in shows its command
local locked = drafts .. "/3.eml"
vim.fn.writefile({ "To: bob@example.com", "", "hi" }, locked)
home()
vim.cmd.edit(locked)
local locked_buf = vim.api.nvim_get_current_buf()
vim.cmd("PostalAgent locked")
local locked_pane = vim.api.nvim_get_current_buf()
if
	not vim.wait(10000, function()
		local said = table.concat(vim.api.nvim_buf_get_lines(locked_pane, 0, -1, false), "\n")
		return said:find("Run `acp-stub login`.", 1, true) ~= nil
	end)
then
	fail("no sign-in shown: " .. vim.inspect(vim.api.nvim_buf_get_lines(locked_pane, 0, -1, false)))
end

vim.cmd("bwipeout! " .. locked_buf)

-- An agent off PATH says what to do, in the pane and in an error
s.notified = {}
vim.notify = function(msg)
	table.insert(s.notified, msg)
end

-- A key setup() does not know is refused by name, not silently dropped
require("postal").setup(vim.tbl_extend("force", setup, { keys = { nope = "x" } }))
if not (s.notified[1] or ""):find("no key named nope", 1, true) then
	fail("an unknown key passed: " .. vim.inspect(s.notified))
end

s.notified = {}
require("postal").setup(setup)

local function chat(name)
	local file = drafts .. "/" .. name .. ".eml"
	vim.fn.writefile({ "To: bob@example.com", "", "hi" }, file)
	home()
	vim.cmd.edit(file)
	local buf = vim.api.nvim_get_current_buf()
	vim.cmd("PostalAgent " .. name)
	return buf, vim.api.nvim_get_current_buf()
end

local absent_buf, absent_pane = chat("absent")
local hint = "absent needs `postal-no-such-agent`, which is not on PATH"
if not said(absent_pane):find(hint, 1, true) or not (s.notified[1] or ""):find(hint, 1, true) then
	fail("no hint for a missing agent: " .. vim.inspect(said(absent_pane)) .. " " .. vim.inspect(s.notified))
end

vim.cmd("bwipeout! " .. absent_buf)

-- Off PATH, an agent with an npx package runs through npx
local bin = vim.fn.tempname()
vim.fn.mkdir(bin, "p")
vim.fn.writefile({
	"#!/bin/sh",
	'[ "$1 $2" = "-y postal-acp-package" ] || exit 3',
	("exec '%s' --clean -l '%s' locked"):format(vim.v.progpath, acp_stub),
}, bin .. "/npx")
vim.fn.setfperm(bin .. "/npx", "rwxr-xr-x")
vim.env.PATH = bin .. ":" .. vim.env.PATH
local fetched_buf, fetched_pane = chat("fetched")
if
	not vim.wait(10000, function()
		return said(fetched_pane):find("Run `acp-stub login`.", 1, true) ~= nil
	end) or not said(fetched_pane):find("npx fetches postal-acp-package", 1, true)
then
	fail("npx did not stand in: " .. vim.inspect(said(fetched_pane)))
end

vim.cmd("bwipeout! " .. fetched_buf)

s.hint = hint
