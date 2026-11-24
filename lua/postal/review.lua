-- The review of an agent's edit to a Draft's body, hunk by hunk: each is
-- marked over the buffer until the user accepts or rejects it (ADR 0019).
local body = require("postal.body")
local hunks = require("postal.hunks")
local keys = require("postal.keys")

local M = {}

local function under(accept)
	return function(review)
		review:under_cursor(accept)
	end
end

local function all(accept)
	return function(review)
		review:settle(accept)
	end
end

--- The keys a review maps in its Draft's buffer while it is open, by their
--- name in `postal.keys`; `short` names one in the pane's winbar.
M.keys = {
	{ name = "accept", short = "accept", desc = "accept the hunk under the cursor", run = under(true) },
	{ name = "reject", short = "reject", desc = "reject the hunk under the cursor", run = under(false) },
	{ name = "accept_all", short = "all", desc = "accept every hunk", run = all(true) },
	{ name = "reject_all", short = "none", desc = "reject every hunk", run = all(false) },
}

local NS = vim.api.nvim_create_namespace("postal-review")

local Review = {}
Review.__index = Review

--- Reviews the agent's `proposed` body for Draft buffer `draft` against
--- `sent`, the body its turn was sent with; nil when there is nothing to
--- review. `changed(summary, undone)` runs on each decision, `summary` nil
--- at the end, `undone` when an undo took the agent's edit back.
function M.new(draft, sent, proposed, changed)
	local found = hunks.between(sent, body.get(draft), proposed)
	if #found == 0 then
		return nil
	end

	local self = setmetatable({ draft = draft, changed = changed, hunks = found }, Review)
	self:place()
	self:map()
	self:watch()
	return self
end

-- Puts the clean hunks into the buffer as one undoable change, then marks
-- every hunk.
function Review:place()
	local buf, first = self.draft, nil
	vim.api.nvim_buf_call(buf, function()
		-- A fresh undo step, so that an undo of the agent's edit leaves the
		-- user's own typing before it.
		vim.o.undolevels = vim.o.undolevels
		local row, added = body.row(buf)
		first = row
		local joined = added
		for n = #self.hunks, 1, -1 do
			local hunk = self.hunks[n]
			if not hunk.conflict then
				if joined then
					pcall(vim.cmd.undojoin)
				end

				joined = true
				vim.api.nvim_buf_set_lines(buf, first + hunk.at, first + hunk.at + #hunk.old, false, hunk.new)
			end
		end

		if joined then
			self.placed = vim.fn.undotree().seq_cur
		end
	end)

	local shift = 0
	for _, hunk in ipairs(self.hunks) do
		self:mark(hunk, first + hunk.at + shift)
		if not hunk.conflict then
			shift = shift + #hunk.new - #hunk.old
		end
	end
end

-- Virtual lines never wrap: cuts `line` at spaces into pieces at most
-- `width` cells wide. A window resized later keeps the old cuts.
local function fold(line, width)
	local pieces = {}
	while vim.fn.strdisplaywidth(line) > width do
		local chars = 1
		while vim.fn.strdisplaywidth(vim.fn.strcharpart(line, 0, chars + 1)) <= width do
			chars = chars + 1
		end

		local head = vim.fn.strcharpart(line, 0, chars)
		local space = head:match(".*() ")
		if space and space > 1 then
			table.insert(pieces, line:sub(1, space - 1))
			line = line:sub(space + 1)
		else
			table.insert(pieces, head)
			line = line:sub(#head + 1)
		end
	end

	table.insert(pieces, line)
	return pieces
end

-- Marks the lines `hunk` shows from `row` on, the lines it would replace
-- as virtual ones over them.
function Review:mark(hunk, row)
	local shown = hunk.conflict and hunk.old or hunk.new
	local over = hunk.conflict and hunk.new or hunk.old
	local group = hunk.conflict and "PostalReviewAdd" or "PostalReviewDelete"
	local mark = { virt_lines_above = true, virt_lines = {} }
	local win = vim.fn.bufwinid(self.draft)
	local width = win == -1 and math.huge
		or math.max(20, vim.api.nvim_win_get_width(win) - vim.fn.getwininfo(win)[1].textoff)
	for _, line in ipairs(over) do
		for _, piece in ipairs(fold(line, width)) do
			table.insert(mark.virt_lines, { { piece, group } })
		end
	end

	if #shown > 0 then
		mark.end_row = row + #shown
		mark.end_col = 0
		mark.end_right_gravity = false
		mark.hl_group = hunk.conflict and "PostalReviewConflict" or "PostalReviewAdd"
		mark.hl_eol = true
	end

	local label = { " conflict", "PostalReviewConflict" }
	if hunk.conflict and #over > 0 then
		table.insert(mark.virt_lines[1], label)
	elseif hunk.conflict then
		mark.virt_text = { label }
	end

	-- A mark past the last line would sit on it: below it, then.
	hunk.tail = row == vim.api.nvim_buf_line_count(self.draft)
	if hunk.tail then
		mark.virt_lines_above = false
		row = row - 1
	end

	hunk.mark = vim.api.nvim_buf_set_extmark(self.draft, NS, row, 0, mark)
end

-- The hunk's lines now: its first row and how many follow.
function Review:range(hunk)
	local row, _, details = unpack(vim.api.nvim_buf_get_extmark_by_id(self.draft, NS, hunk.mark, { details = true }))
	if hunk.tail then
		return row + 1, 0
	end

	return row, details.end_row and details.end_row - row or 0
end

-- Ends the review once an undo goes past the placed hunks: their marks
-- would collapse, and a decision would write over the wrong rows.
function Review:watch()
	if not self.placed then
		return
	end

	vim.api.nvim_buf_attach(self.draft, false, {
		on_lines = function()
			if self.over then
				return true
			end

			vim.schedule(function()
				local seq = vim.api.nvim_buf_call(self.draft, function()
					return vim.fn.undotree().seq_cur
				end)

				if not self.over and seq < self.placed then
					vim.api.nvim_buf_clear_namespace(self.draft, NS, 0, -1)
					self.hunks = {}
					self:report(true)
				end
			end)
		end,
	})
end

function Review:map()
	self.mapped = keys.bound(vim.tbl_map(function(key)
		return vim.tbl_extend("force", key, {
			run = function()
				key.run(self)
			end,
		})
	end, M.keys))

	keys.map(self.draft, self.mapped)
end

function Review:unmap()
	keys.unmap(self.draft, self.mapped)
end

-- Takes the agent's lines, or the user's, for one hunk and drops its mark.
function Review:decide(hunk, accept)
	local row, count = self:range(hunk)
	vim.api.nvim_buf_del_extmark(self.draft, NS, hunk.mark)
	if hunk.conflict == accept then
		vim.api.nvim_buf_set_lines(self.draft, row, row + count, false, accept and hunk.new or hunk.old)
	end

	for n, other in ipairs(self.hunks) do
		if other == hunk then
			table.remove(self.hunks, n)
			break
		end
	end
end

function Review:under_cursor(accept)
	local cursor = vim.api.nvim_win_get_cursor(0)[1] - 1
	for _, hunk in ipairs(self.hunks) do
		local row, count = self:range(hunk)
		local shown = hunk.tail and row - 1 or row
		if cursor >= shown and cursor < shown + math.max(count, 1) then
			self:decide(hunk, accept)
			self:report()
			return
		end
	end

	vim.notify("postal: no hunk of the agent's under the cursor", vim.log.levels.WARN)
end

--- Accepts, or rejects, every hunk left; `quiet` tells no one.
function Review:settle(accept, quiet)
	for n = #self.hunks, 1, -1 do
		self:decide(self.hunks[n], accept)
	end

	if quiet then
		self.over = true
		self:unmap()
	else
		self:report()
	end
end

--- How many hunks are left, and how to decide them.
function Review:summary()
	local conflicts = #vim.tbl_filter(function(hunk)
		return hunk.conflict
	end, self.hunks)

	local left = #self.hunks
	local text = ("%d %s left"):format(left, left == 1 and "hunk" or "hunks")
	if conflicts > 0 then
		text = text .. (" (%d %s)"):format(conflicts, conflicts == 1 and "conflict" or "conflicts")
	end

	return text
end

function Review:report(undone)
	if #self.hunks > 0 then
		self.changed(self:summary())
		return
	end

	self.over = true
	self:unmap()
	self.changed(nil, undone)
end

return M
