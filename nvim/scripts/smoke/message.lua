-- A Message opens read-only: the agent reads it with its Thread, and its
-- links open from their markers.
local s = ...
local fail = s.fail
local said = s.said
local home = s.home
local prompts = s.prompts
local answers = s.answers

-- A Message opened to read cannot be changed, by the user or the agent,
-- and the agent reads it whole
local messages = vim.fn.tempname() .. "/postal/messages/me@example.com"
vim.fn.mkdir(messages, "p")
local message_text = {
	"From: Ada <ada@example.com>",
	"Subject: Weekly notes",
	"",
	"Read the notes[1], then [2].",
	"",
	"[1] https://example.com/notes",
	"[2] https://example.com/next",
}
vim.fn.writefile(message_text, messages .. "/7.eml")
home()
vim.cmd.edit(messages .. "/7.eml")
local message_buf = vim.api.nvim_get_current_buf()
if vim.bo.modifiable or not vim.bo.readonly or not vim.wo.wrap or vim.wo.spell then
	fail("the Message can be changed, or is not wrapped")
end

local before_message = #prompts()
vim.cmd("PostalAgent")
local message_pane = vim.api.nvim_get_current_buf()
-- What was sent in another chat comes back, newest first, and past the
-- newest what was typed
vim.api.nvim_buf_set_lines(0, -2, -1, false, { "half typed" })
local recalled = {}
for _, lhs in ipairs({ "<C-p>", "<C-p>", "<C-n>", "<C-n>", "<C-n>" }) do
	vim.cmd.normal(vim.keycode(lhs))
	table.insert(recalled, vim.api.nvim_buf_get_lines(0, -2, -1, false)[1])
end

if not vim.deep_equal(recalled, { "hang", "after", "hang", "half typed", "half typed" }) then
	fail("the history typed " .. vim.inspect(recalled))
end
vim.api.nvim_buf_set_lines(0, -2, -1, false, { "what is it about?" })
vim.cmd.normal(vim.keycode("<CR>"))
if not vim.wait(10000, function()
	return said(message_pane):find("It is about Weekly notes.", 1, true) ~= nil
end) then
	fail("the agent did not read the Message: " .. vim.inspect(said(message_pane)))
end

local asked = prompts()[before_message + 1].params.prompt
local message_blocks = vim.tbl_map(function(block)
	return block.resource and block.resource.uri:match("^[%w-]+") or block.text
end, asked)

-- It goes with its Thread, which the stub gives whatever it is asked of
local want_blocks = { asked[1].text, "postal-message", "postal-thread", "what is it about?" }
if os.getenv("POSTAL_BIN") then
	table.remove(want_blocks, 3)
end

if not vim.deep_equal(message_blocks, want_blocks) or not asked[1].text:find("read_message", 1, true) then
	fail("the Message's prompt holds " .. vim.inspect(asked))
end

if asked[2].resource.text ~= table.concat(message_text, "\n") .. "\n" then
	fail("the Message resource is " .. vim.inspect(asked[2].resource.text))
end

local message_outcomes = vim.tbl_map(function(message)
	return message.result.outcome.optionId
end, vim.list_slice(answers(), #answers() - 1))

if not vim.deep_equal(message_outcomes, { "no", "yes" }) then
	fail("over a Message the agent was answered " .. vim.inspect(message_outcomes))
end

-- `gx` on a link's marker opens its URL, listed at the end as Postal lays
-- a Message out, and :PostalLinks lists each at its line
local opened_urls = {}
local ui_open = vim.ui.open
vim.ui.open = function(url)
	table.insert(opened_urls, url)
end

home()
vim.cmd.buffer(message_buf)
for _, at in ipairs({ { 4, #"Read the notes" }, { 4, #"Read the notes[1], then [2" }, { 7, #"[2] https" } }) do
	vim.api.nvim_win_set_cursor(0, at)
	vim.cmd.normal("gx")
end

vim.ui.open = ui_open
local want_urls = { "https://example.com/notes", "https://example.com/next", "https://example.com/next" }
if not vim.deep_equal(opened_urls, want_urls) then
	fail("gx opened " .. vim.inspect(opened_urls))
end

vim.cmd("PostalLinks")
local listed = vim.tbl_map(function(item)
	return item.lnum .. " " .. item.text
end, vim.fn.getloclist(0))
vim.cmd.lclose()
if not vim.deep_equal(listed, { "6 [1] https://example.com/notes", "7 [2] https://example.com/next" }) then
	fail(":PostalLinks listed " .. vim.inspect(listed))
end

vim.cmd("bwipeout! " .. message_buf)
if vim.api.nvim_buf_is_valid(message_pane) then
	fail("the pane outlived its Message")
end
