-- The chat pane beside a mail, a markdown buffer laid out like a chat:
-- each message under `## Me` or `## <agent>`, and the user's next one typed
-- under the `## Me` at its bottom.
local bar = require("postal.bar")
local body = require("postal.body")
local help = require("postal.help")
local icons = require("postal.icons")

local M = {}

local NS = vim.api.nvim_create_namespace("postal-pane")
local HINT = vim.api.nvim_create_namespace("postal-pane-hint")
local SPINNER = { "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏" }
local RULE = { { string.rep("─", 200), "PostalRule" } }
local ME = "Me"
local panes = {}
local Pane = {}
Pane.__index = Pane

--- A pane for the mail in buffer `mail`, of `kind` from `postal.kinds`,
--- shown at once; `actions` are `send(text)`, `stop()` and `close()`.
function M.new(mail, kind, actions)
	local self = setmetatable({ mail = mail, kind = kind, actions = actions }, Pane)
	local function submit()
		self:submit()
	end

	-- The key that lists every key, at the winbar's right.
	self.more = {
		modes = "n",
		lhs = "?",
		short = "keys",
		desc = "list the keys",
		run = function()
			local groups = { { "Chat", self.keys } }
			if kind.review then
				table.insert(groups, { "Review, in the " .. kind.noun, kind.review })
			end

			help.open(vim.api.nvim_get_current_win(), groups)
		end,
	}

	-- `short` puts a key on the winbar.
	self.keys = {
		{ modes = "n", lhs = "<CR>", short = "send", desc = "send what is typed under the last Me", run = submit },
		{ modes = { "n", "i" }, lhs = "<C-s>", desc = "send, from insert mode too", run = submit },
		{ modes = "n", lhs = "q", short = "stop", desc = "stop the agent's turn", run = actions.stop },
		{ modes = "n", lhs = "<C-c>", short = "end", desc = "end the chat and close the pane", run = actions.close },
		self.more,
	}

	self:status()
	self:show()
	return self
end

function Pane:buffer()
	if self.buf and vim.api.nvim_buf_is_valid(self.buf) then
		return self.buf
	end

	local buf = vim.api.nvim_create_buf(false, true)
	self.buf, panes[buf] = buf, self
	vim.api.nvim_create_autocmd("BufWipeout", {
		buffer = buf,
		callback = function()
			panes[buf] = nil
		end,
	})

	self.speaker, self.open = nil, nil
	vim.bo[self.buf].filetype = "markdown"
	local glyphs = vim.tbl_map(function(kind)
		return (icons.get(kind))
	end, { "ok", "denied", "context" })

	-- What a wrapped line hangs under: a list's marker, a quote's, a glyph.
	vim.bo[self.buf].formatlistpat = [[^\s*\([-*+>]\|\d\+[.)]\|]] .. table.concat(glyphs, [[\|]]) .. [[\)\s\+]]
	-- Highlights and conceals markdown even where no config starts treesitter.
	pcall(vim.treesitter.start, self.buf, "markdown")
	local name = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(self.mail), ":t")
	vim.api.nvim_buf_set_name(self.buf, "postal-agent://" .. name .. "#" .. self.buf)
	vim.api.nvim_buf_set_lines(self.buf, 0, -1, false, { "## " .. ME, "" })
	-- The input's header: whatever the pane adds goes in above it.
	self.input = vim.api.nvim_buf_set_extmark(self.buf, NS, 0, 0, { virt_lines = { RULE }, virt_lines_above = true })
	self:label(0, ME, "me")
	for _, key in ipairs(self.keys) do
		vim.keymap.set(key.modes, key.lhs, key.run, { buffer = self.buf, desc = key.desc })
	end

	vim.api.nvim_create_autocmd({ "TextChanged", "TextChangedI" }, {
		buffer = self.buf,
		callback = function()
			self:placeholder()
		end,
	})

	return self.buf
end

--- Says how to send over the input while it is empty.
function Pane:placeholder()
	vim.api.nvim_buf_clear_namespace(self.buf, HINT, 0, -1)
	local row = self:bottom() + 1
	local lines = vim.api.nvim_buf_get_lines(self.buf, row, -1, false)
	if #lines > 0 and vim.trim(table.concat(lines)) == "" then
		local text = { { "Type here, <C-s> to send", "PostalDim" } }
		vim.api.nvim_buf_set_extmark(self.buf, HINT, row, 0, { virt_text = text, virt_text_pos = "overlay" })
	end
end

--- Opens the pane's window right of the mail's, unless one shows it; with
--- `focus`, puts the cursor at the end of its input.
function Pane:show(focus)
	local buf = self:buffer()
	local win = vim.fn.bufwinid(buf)
	if win == -1 then
		local beside = vim.fn.bufwinid(self.mail)
		-- A third of the screen, at least 60 columns, but never over half.
		local width = math.min(math.max(60, math.floor(vim.o.columns / 3)), math.floor(vim.o.columns / 2))
		win = vim.api.nvim_open_win(buf, false, { split = "right", win = beside ~= -1 and beside or 0, width = width })
		vim.wo[win].winfixwidth = true
		vim.wo[win].wrap = true
		vim.wo[win].linebreak = true
		vim.wo[win].breakindent = true
		vim.wo[win].breakindentopt = "list:-1"
		vim.wo[win].number = false
		vim.wo[win].relativenumber = false
		vim.wo[win].foldenable = false
		vim.wo[win].list = false
		-- A blank gutter, the pane's left margin; it draws no sign.
		vim.wo[win].signcolumn = "yes:1"
		vim.wo[win].statuscolumn = "  "
		vim.wo[win].conceallevel = 2
		vim.wo[win].concealcursor = "nc"
		vim.wo[win].winbar = "%{%v:lua.require'postal.pane'.winbar(" .. buf .. ")%}"
		vim.wo[win].statusline = "%{%v:lua.require'postal.pane'.statusline(" .. buf .. ")%}"
	end

	if focus then
		vim.api.nvim_set_current_win(win)
		vim.api.nvim_win_set_cursor(win, { vim.api.nvim_buf_line_count(buf), 0 })
	end
end

--- Shows the chat's keys on the winbar or, with `summary` of a review,
--- it and the review's keys.
function Pane:status(summary)
	self.summary = summary
	vim.cmd.redrawstatus({ bang = true })
end

--- The winbar of the pane on buffer `buf`, in the window it is drawn in.
function M.winbar(buf)
	local self = panes[buf]
	if not self then
		return ""
	end

	local keys = self.summary and self.kind.review or self.keys
	return bar.winbar(vim.api.nvim_win_get_width(0), keys, self.more, self.summary)
end

--- Names agent `who`, on the statusline and over what it says.
function Pane:title(who)
	self.agent = who
end

--- The statusline of the pane on buffer `buf`.
function M.statusline(buf)
	local self = panes[buf]
	if not self or not vim.api.nvim_buf_is_valid(self.mail) then
		return ""
	end

	local subject = body.headless(self.mail) and self.kind.noun or body.header(self.mail, "Subject")
	return bar.statusline(self.agent, subject)
end

--- Shows the `## who` header at `row` in the look of `role`, `me` or
--- `agent`: its icon, and the name in its colour.
function Pane:label(row, who, role)
	local glyph, group = icons.get(role)
	self:sign(row, 3, glyph .. " ", group)
	self:paint(row, 3, 3 + #who, group)
end

--- Hides the first `width` columns of `row` behind `sign`.
function Pane:sign(row, width, sign, group)
	vim.api.nvim_buf_set_extmark(self.buf, NS, row, 0, {
		end_col = width,
		conceal = "",
		virt_text = { { sign, group } },
		virt_text_pos = "inline",
	})
end

--- Colours columns `from` to `to` of `row` over markdown's own colours.
function Pane:paint(row, from, to, group)
	vim.api.nvim_buf_set_extmark(self.buf, NS, row, from, { end_col = to, hl_group = group, priority = 200 })
end

--- The 0-based row of the input's header.
function Pane:bottom()
	return vim.api.nvim_buf_get_extmark_by_id(self.buf, NS, self.input, {})[1]
end

--- The 0-based row of the open block's last line.
function Pane:tail()
	return vim.api.nvim_buf_get_extmark_by_id(self.buf, NS, self.open.mark, {})[1]
end

--- Runs `edit` on the buffer, then scrolls each window on the pane but the
--- user's to its end; once the pane is closed, does nothing.
function Pane:write(edit)
	if self.closed then
		return
	end

	edit(self:buffer())
	self:placeholder()
	for _, win in ipairs(vim.fn.win_findbuf(self.buf)) do
		if win ~= vim.api.nvim_get_current_win() then
			vim.api.nvim_win_set_cursor(win, { vim.api.nvim_buf_line_count(self.buf), 0 })
		end
	end
end

--- Adds `lines` above the input, a blank line after them, and opens them as
--- a block of `kind` that may grow in place, unless nil. Returns their row.
function Pane:block(kind, lines)
	local row = self:bottom()
	vim.api.nvim_buf_set_lines(self.buf, row, row, false, vim.list_extend(vim.deepcopy(lines), { "" }))
	self.open = kind and { kind = kind, mark = vim.api.nvim_buf_set_extmark(self.buf, NS, row + #lines - 1, 0, {}) }
	return row
end

--- Moves the open block's end to `row`.
function Pane:grow(row)
	vim.api.nvim_buf_set_extmark(self.buf, NS, row, 0, { id = self.open.mark })
end

--- A `## who` header in the look of `role`, with the rule over it unless
--- it is the first line.
function Pane:header(who, role)
	local row = self:block(nil, { "## " .. who })
	if row > 0 then
		vim.api.nvim_buf_set_extmark(self.buf, NS, row, 0, { virt_lines = { RULE }, virt_lines_above = true })
	end

	self:label(row, who, role)
	self.speaker = who
	return row
end

--- The user's message, taken from the input.
function Pane:submit()
	local row = self:bottom()
	local text = vim.trim(table.concat(vim.api.nvim_buf_get_lines(self.buf, row + 1, -1, false), "\n"))
	if text == "" then
		return
	end

	self:write(function(buf)
		vim.api.nvim_buf_set_lines(buf, row + 1, -1, false, { "" })
		self:header(ME, "me")
		self:block(nil, vim.split(text, "\n", { plain = true }))
	end)

	self.actions.send(text)
end

--- Adds a chunk of the agent's message, under its header.
function Pane:stream(text)
	self:write(function(buf)
		if self.speaker ~= self.agent then
			self:header(self.agent, "agent")
		end

		if not (self.open and self.open.kind == "text") then
			self:block("text", { "" })
		end

		local row = self:tail()
		local pieces = vim.split(text, "\n", { plain = true })
		pieces[1] = vim.api.nvim_buf_get_lines(buf, row, row + 1, false)[1] .. pieces[1]
		vim.api.nvim_buf_set_lines(buf, row, row + 1, false, pieces)
		self:grow(row + #pieces - 1)
	end)
end

--- One dim line, the icon of `kind` (`ok`, `denied`, `context`) before
--- `text`; a run of them has no blank lines between.
function Pane:dim(kind, text)
	self:write(function(buf)
		local glyph, group = icons.get(kind)
		local line = { glyph .. " " .. text:gsub("\n", " ") }
		local row
		if self.open and self.open.kind == "dim" then
			row = self:tail() + 1
			vim.api.nvim_buf_set_lines(buf, row, row, false, line)
			self:grow(row)
		else
			row = self:block("dim", line)
		end

		vim.api.nvim_buf_set_extmark(buf, NS, row, 0, { end_row = row + 1, hl_group = "PostalDim", hl_eol = true })
		self:paint(row, 0, #glyph, group)
	end)
end

--- What the plugin itself says, as a quote.
function Pane:notice(text)
	self:write(function()
		local glyph, group = icons.get("notice")
		local lines = vim.split(text, "\n", { plain = true })
		local row = self:block(
			nil,
			vim.tbl_map(function(line)
				return "> " .. line
			end, lines)
		)
		for i = 0, #lines - 1 do
			self:sign(row + i, 2, (i == 0 and glyph or " ") .. " ", group)
		end
	end)
end

--- Opens the agent's turn: its header, spinning until `end_turn`.
function Pane:begin_turn()
	self:end_turn()
	self:write(function(buf)
		self.spin = vim.api.nvim_buf_set_extmark(buf, NS, self:header(self.agent, "agent"), 0, {})
	end)

	if self.closed then
		return
	end

	local frame = 0
	self.timer = vim.uv.new_timer()
	self.timer:start(0, 80, function()
		vim.schedule(function()
			-- A pane whose buffer was wiped mid-turn has lost the header.
			local row = self.timer and vim.api.nvim_buf_get_extmark_by_id(self:buffer(), NS, self.spin, {})[1]
			if not row then
				return
			end

			frame = frame % #SPINNER + 1
			local spin = { { " " .. SPINNER[frame], "PostalAgent" } }
			vim.api.nvim_buf_set_extmark(self.buf, NS, row, 0, { id = self.spin, virt_text = spin })
		end)
	end)
end

function Pane:end_turn()
	if self.timer then
		self.timer:stop()
		self.timer:close()
		self.timer = nil
		vim.api.nvim_buf_del_extmark(self:buffer(), NS, self.spin)
	end
end

--- Closes the pane for good: what the agent says after is dropped.
function Pane:close()
	self:end_turn()
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
