-- The chat pane beside a Draft, a markdown buffer laid out like a chat:
-- each message under `## Me` or `## <agent>`, and the user's next one typed
-- under the `## Me` at its bottom.
local M = {}

local NS = vim.api.nvim_create_namespace("postal-pane")
local SPINNER = { "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏" }
local RULE = { { string.rep("─", 200), "WinSeparator" } }
local HINT = "<CR> send · q stop · ? keys"
local KEYS = {
	{ "<CR>, <C-s>", "send what is typed under the last ## Me" },
	{ "q", "stop the agent's turn" },
	{ "<C-c>", "end the chat and close the pane" },
}

local Pane = {}
Pane.__index = Pane

--- A pane for the Draft in buffer `draft` and agent `name`, shown at once;
--- `actions` are `send(text)`, `stop()` and `close()`.
function M.new(draft, name, actions)
	local self = setmetatable({ draft = draft, name = name, actions = actions, winbar = " " .. name }, Pane)
	self:show()
	return self
end

function Pane:map(modes, lhs, action, desc)
	vim.keymap.set(modes, lhs, action, { buffer = self.buf, desc = desc })
end

function Pane:buffer()
	if self.buf and vim.api.nvim_buf_is_valid(self.buf) then
		return self.buf
	end

	self.buf = vim.api.nvim_create_buf(false, true)
	self.speaker, self.last = nil, nil
	vim.bo[self.buf].filetype = "markdown"
	local name = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(self.draft), ":t")
	vim.api.nvim_buf_set_name(self.buf, "postal-agent://" .. name .. "#" .. self.buf)
	vim.api.nvim_buf_set_lines(self.buf, 0, -1, false, { "## Me", "" })
	-- The input's header: whatever the pane adds goes in above it.
	self.input = vim.api.nvim_buf_set_extmark(self.buf, NS, 0, 0, {
		virt_text = { { "  " .. HINT, "Comment" } },
		virt_lines = { RULE },
		virt_lines_above = true,
	})

	self:map({ "n", "i" }, "<C-s>", function()
		self:submit()
	end, "Send to the agent")
	self:map("n", "<CR>", function()
		self:submit()
	end, "Send to the agent")
	self:map("n", "q", self.actions.stop, "Stop the agent's turn")
	self:map("n", "<C-c>", self.actions.close, "End the chat")
	self:map("n", "?", function()
		local lines = vim.tbl_map(function(key)
			return { ("%-12s %s\n"):format(key[1], key[2]) }
		end, KEYS)
		vim.api.nvim_echo(lines, false, {})
	end, "List the chat's keys")
	return self.buf
end

--- Opens the pane's window right of the Draft's, unless one shows it; with
--- `focus`, puts the cursor at the end of its input.
function Pane:show(focus)
	local buf = self:buffer()
	local win = vim.fn.bufwinid(buf)
	if win == -1 then
		local beside = vim.fn.bufwinid(self.draft)
		win = vim.api.nvim_open_win(buf, false, { split = "right", win = beside ~= -1 and beside or 0 })
		vim.wo[win].wrap = true
		vim.wo[win].linebreak = true
		vim.wo[win].breakindent = true
		vim.wo[win].number = false
		vim.wo[win].relativenumber = false
		vim.wo[win].signcolumn = "no"
		vim.wo[win].winbar = self.winbar
	end

	if focus then
		vim.api.nvim_set_current_win(win)
		vim.api.nvim_win_set_cursor(win, { vim.api.nvim_buf_line_count(buf), 0 })
	end
end

--- The winbar after the agent's name; empty leaves the name alone.
function Pane:status(text)
	local line = " " .. self.name .. (text ~= "" and " · " .. text or "")
	-- A narrow pane cuts the end, the keys, rather than the name.
	self.winbar = line:gsub("%%", "%%%%") .. "%<"
	local win = self.buf and vim.fn.bufwinid(self.buf) or -1
	if win ~= -1 then
		vim.wo[win].winbar = self.winbar
	end
end

--- The 0-based row of the input's header.
function Pane:bottom()
	return vim.api.nvim_buf_get_extmark_by_id(self:buffer(), NS, self.input, {})[1]
end

--- Scrolls each window on the pane to its end, but the one the user is in.
function Pane:follow()
	for _, win in ipairs(vim.fn.win_findbuf(self.buf)) do
		if win ~= vim.api.nvim_get_current_win() then
			vim.api.nvim_win_set_cursor(win, { vim.api.nvim_buf_line_count(self.buf), 0 })
		end
	end
end

--- Adds `lines` and a blank line above the input; returns their first row.
function Pane:put(lines)
	local row = self:bottom()
	vim.api.nvim_buf_set_lines(self.buf, row, row, false, vim.list_extend(vim.deepcopy(lines), { "" }))
	self:follow()
	return row
end

--- A `## who` header with the rule over it, unless it is the first line.
function Pane:header(who)
	local row = self:put({ "## " .. who })
	if row > 0 then
		vim.api.nvim_buf_set_extmark(self.buf, NS, row, 0, { virt_lines = { RULE }, virt_lines_above = true })
	end

	self.speaker, self.last = who, nil
	return row
end

--- The user's message, taken from the input.
function Pane:submit()
	local row = self:bottom()
	local lines = vim.api.nvim_buf_get_lines(self.buf, row + 1, -1, false)
	local text = vim.trim(table.concat(lines, "\n"))
	if text == "" then
		return
	end

	vim.api.nvim_buf_set_lines(self.buf, row + 1, -1, false, { "" })
	self:header("Me")
	self:put(vim.split(text, "\n", { plain = true }))
	self.actions.send(text)
end

--- Adds a chunk of the agent's message, under its header.
function Pane:stream(text)
	if self.closed then
		return
	end

	if self.speaker ~= self.name then
		self:header(self.name)
	end

	if self.last ~= "text" then
		self:put({ "" })
		self.last = "text"
	end

	local row = self:bottom() - 2
	local pieces = vim.split(text, "\n", { plain = true })
	pieces[1] = vim.api.nvim_buf_get_lines(self.buf, row, row + 1, false)[1] .. pieces[1]
	vim.api.nvim_buf_set_lines(self.buf, row, row + 1, false, pieces)
	self:follow()
end

--- One dim line, `icon` before `text`: a tool call, a denial, a context;
--- a run of them has no blank lines between.
function Pane:dim(icon, text)
	if self.closed then
		return
	end

	local line = { icon .. " " .. text:gsub("\n", " ") }
	local row
	if self.last == "dim" then
		row = self:bottom() - 1
		vim.api.nvim_buf_set_lines(self.buf, row, row, false, line)
	else
		row = self:put(line)
	end

	vim.api.nvim_buf_set_extmark(self.buf, NS, row, 0, { end_row = row + 1, hl_group = "Comment", hl_eol = true })
	self.last = "dim"
	self:follow()
end

--- What the plugin itself says, as a quote.
function Pane:notice(text)
	if self.closed then
		return
	end

	self:put(vim.tbl_map(function(line)
		return "> " .. line
	end, vim.split(text, "\n", { plain = true })))
	self.last = "notice"
end

--- Spins on the agent's header while `on`; a turn's header comes first.
function Pane:busy(on)
	if self.timer then
		self.timer:stop()
		self.timer:close()
		self.timer = nil
		pcall(vim.api.nvim_buf_del_extmark, self.buf, NS, self.spin)
	end

	if not on or self.closed then
		return
	end

	self.spin = vim.api.nvim_buf_set_extmark(self.buf, NS, self:header(self.name), 0, {})
	local frame = 0
	self.timer = vim.uv.new_timer()
	self.timer:start(0, 80, function()
		vim.schedule(function()
			if not self.timer or not vim.api.nvim_buf_is_valid(self.buf) then
				return
			end

			frame = frame % #SPINNER + 1
			local row = vim.api.nvim_buf_get_extmark_by_id(self.buf, NS, self.spin, {})[1]
			local spin = { { " " .. SPINNER[frame], "Comment" } }
			vim.api.nvim_buf_set_extmark(self.buf, NS, row, 0, { id = self.spin, virt_text = spin })
		end)
	end)
end

--- Closes the pane for good: what the agent says after is dropped.
function Pane:close()
	self:busy(false)
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
