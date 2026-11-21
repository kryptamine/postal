-- A float of keys, section by section, centred over a window.
local icons = require("postal.icons")

local M = {}

local NS = vim.api.nvim_create_namespace("postal-help")

--- Opens it over `win`; `sections` are `{ title, keys }`, each key a
--- `{ lhs = ..., desc = ... }`.
--- `q`, `<Esc>`, `?` or leaving it closes it.
function M.open(win, sections)
	local wide = 0
	for _, section in ipairs(sections) do
		for _, key in ipairs(section[2]) do
			wide = math.max(wide, vim.fn.strdisplaywidth(key.lhs))
		end
	end

	local lines, marks = {}, {}
	for i, section in ipairs(sections) do
		if i > 1 then
			table.insert(lines, "")
		end

		table.insert(lines, " " .. section[1])
		table.insert(marks, { #lines - 1, 1, #lines[#lines], "PostalSection" })
		for _, key in ipairs(section[2]) do
			local pad = string.rep(" ", wide - vim.fn.strdisplaywidth(key.lhs))
			table.insert(lines, "   " .. key.lhs .. pad .. "   " .. key.desc .. " ")
			table.insert(marks, { #lines - 1, 3, 3 + #key.lhs, "PostalKey" })
		end
	end

	local width = 0
	for _, line in ipairs(lines) do
		width = math.max(width, vim.fn.strdisplaywidth(line))
	end

	local buf = vim.api.nvim_create_buf(false, true)
	vim.api.nvim_buf_set_lines(buf, 0, -1, false, lines)
	for _, mark in ipairs(marks) do
		vim.api.nvim_buf_set_extmark(buf, NS, mark[1], mark[2], { end_col = mark[3], hl_group = mark[4] })
	end

	vim.bo[buf].modifiable = false
	vim.bo[buf].bufhidden = "wipe"
	local float = vim.api.nvim_open_win(buf, true, {
		relative = "win",
		win = win,
		row = math.max(0, math.floor((vim.api.nvim_win_get_height(win) - #lines) / 2) - 1),
		col = math.max(0, math.floor((vim.api.nvim_win_get_width(win) - width) / 2) - 1),
		width = width,
		height = #lines,
		style = "minimal",
		border = "rounded",
		title = " " .. icons.get("keys") .. " Keys ",
		title_pos = "center",
	})

	local function close()
		pcall(vim.api.nvim_win_close, float, true)
	end

	for _, lhs in ipairs({ "q", "<Esc>", "?" }) do
		vim.keymap.set("n", lhs, close, { buffer = buf, nowait = true })
	end

	vim.api.nvim_create_autocmd("WinLeave", { buffer = buf, once = true, callback = close })
end

return M
