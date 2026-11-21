-- A Draft's body in its buffer: every line after the blank one that ends
-- the headers, as text ending in a newline.
local M = {}

-- The 0-based row the body starts at, and whether the headers end in a
-- blank line; without one, the line count.
local function start(buf)
	local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
	for n, line in ipairs(lines) do
		if line == "" then
			return n, true
		end
	end

	return #lines, false
end

function M.get(buf)
	local lines = vim.api.nvim_buf_get_lines(buf, (start(buf)), -1, false)
	if #lines == 0 then
		return ""
	end

	return table.concat(lines, "\n") .. "\n"
end

--- Replaces the body with `text` in one undoable change, the headers as
--- they are.
function M.set(buf, text)
	local first, ended = start(buf)
	local lines = vim.split((text:gsub("\n$", "")), "\n", { plain = true })
	if not ended then
		table.insert(lines, 1, "")
	end

	vim.api.nvim_buf_set_lines(buf, first, -1, false, lines)
end

return M
