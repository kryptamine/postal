-- A mail's text in its buffer, as text ending in a newline: the body is
-- every line after the blank one that ends the headers.
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

--- The value of header `name`, in any case, or nil when there is none.
function M.header(buf, name)
	local prefix = name:lower() .. ":"
	for _, line in ipairs(vim.api.nvim_buf_get_lines(buf, 0, (start(buf)), false)) do
		if line:sub(1, #prefix):lower() == prefix then
			return vim.trim(line:sub(#prefix + 1))
		end
	end
end

--- Every line, the headers' too.
function M.whole(buf)
	return table.concat(vim.api.nvim_buf_get_lines(buf, 0, -1, false), "\n") .. "\n"
end

function M.get(buf)
	local lines = vim.api.nvim_buf_get_lines(buf, (start(buf)), -1, false)
	if #lines == 0 then
		return ""
	end

	return table.concat(lines, "\n") .. "\n"
end

--- Adds the blank line that ends the headers when there is none, and
--- returns the 0-based row the body starts at and whether it added one.
function M.row(buf)
	local first, ended = start(buf)
	if not ended then
		vim.api.nvim_buf_set_lines(buf, first, first, false, { "" })
		first = first + 1
	end

	return first, not ended
end

return M
