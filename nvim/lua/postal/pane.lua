-- The chat pane beside a Draft: what the user asked and what the agent
-- says, read-only, its messages streaming in as they come.
local M = {}

local Pane = {}
Pane.__index = Pane

--- A pane for the Draft in buffer `draft`, shown at once. `ask` runs on
--- `<CR>` in it.
function M.new(draft, ask)
	local self = setmetatable({ draft = draft, ask = ask, streaming = false }, Pane)
	self:show()
	return self
end

function Pane:buffer()
	if self.buf and vim.api.nvim_buf_is_valid(self.buf) then
		return self.buf
	end

	self.buf = vim.api.nvim_create_buf(false, true)
	self.streaming = false
	vim.bo[self.buf].filetype = "markdown"
	vim.bo[self.buf].modifiable = false
	local name = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(self.draft), ":t")
	vim.api.nvim_buf_set_name(self.buf, "postal-agent://" .. name .. "#" .. self.buf)
	vim.keymap.set("n", "<CR>", self.ask, { buffer = self.buf, desc = "Ask the agent" })
	return self.buf
end

--- Opens the pane's window right of the Draft's, unless one shows it.
function Pane:show()
	local buf = self:buffer()
	if vim.fn.bufwinid(buf) ~= -1 then
		return
	end

	local beside = vim.fn.bufwinid(self.draft)
	local win = vim.api.nvim_open_win(buf, false, { split = "right", win = beside ~= -1 and beside or 0 })
	vim.wo[win].wrap = true
	vim.wo[win].linebreak = true
	vim.wo[win].winbar = self.winbar or ""
end

--- A line atop the pane, kept until the next one; empty hides it.
function Pane:status(text)
	self.winbar = text
	local win = self.buf and vim.fn.bufwinid(self.buf) or -1
	if win ~= -1 then
		vim.wo[win].winbar = text
	end
end

local function blank(buf)
	return vim.api.nvim_buf_line_count(buf) == 1 and vim.api.nvim_buf_get_lines(buf, 0, 1, false)[1] == ""
end

function Pane:append(lines)
	local buf = self:buffer()
	vim.bo[buf].modifiable = true
	-- A new buffer's one empty line is replaced, not followed.
	local start = blank(buf) and 0 or vim.api.nvim_buf_line_count(buf)
	vim.api.nvim_buf_set_lines(buf, start, -1, false, lines)
	vim.bo[buf].modifiable = false
	self:follow()
end

function Pane:follow()
	local win = vim.fn.bufwinid(self.buf)
	if win ~= -1 then
		vim.api.nvim_win_set_cursor(win, { vim.api.nvim_buf_line_count(self.buf), 0 })
	end
end

--- A paragraph of its own, `text` possibly holding newlines.
function Pane:say(text)
	if self.closed then
		return
	end

	self.streaming = false
	local lines = vim.split(text, "\n", { plain = true })
	if not blank(self:buffer()) then
		table.insert(lines, 1, "")
	end

	self:append(lines)
end

--- Adds a chunk of the agent's message; a message's first chunk opens a
--- paragraph headed `who`.
function Pane:stream(who, text)
	if self.closed then
		return
	end

	self:buffer()
	if not self.streaming then
		self:say(who .. ": ")
		self.streaming = true
	end

	local buf = self:buffer()
	local pieces = vim.split(text, "\n", { plain = true })
	local row = vim.api.nvim_buf_line_count(buf) - 1
	local last = vim.api.nvim_buf_get_lines(buf, row, row + 1, false)[1]
	pieces[1] = last .. pieces[1]
	vim.bo[buf].modifiable = true
	vim.api.nvim_buf_set_lines(buf, row, row + 1, false, pieces)
	vim.bo[buf].modifiable = false
	self:follow()
end

--- Closes the pane for good: what the agent says after is dropped.
function Pane:close()
	self.closed = true
	if not (self.buf and vim.api.nvim_buf_is_valid(self.buf)) then
		return
	end

	for _, win in ipairs(vim.fn.win_findbuf(self.buf)) do
		-- The last window stays, on another buffer once this one is gone.
		pcall(vim.api.nvim_win_close, win, true)
	end

	vim.api.nvim_buf_delete(self.buf, { force = true })
end

return M
