-- Which header of a Postal Draft a line belongs to.
local M = {}

--- The lowercased name of the header line `row` (1-based) of buffer `buf`
--- is in, a folded line counting as its header's, and that header's row;
--- nil in the body or before any header.
function M.at(buf, row)
	local lines = vim.api.nvim_buf_get_lines(buf, 0, row, false)
	for n = 1, row do
		if lines[n] == "" then
			return nil
		end
	end

	local header = row
	while header >= 1 and lines[header]:match("^[ \t]") do
		header = header - 1
	end

	local name = header >= 1 and lines[header]:match("^([^:%s]+):")
	if not name then
		return nil
	end

	return name:lower(), header
end

--- Each header of `buf`, in order: its lowercased `name`, its `row`
--- (1-based) and its `value`, a folded line joined on, as Postal reads it.
function M.fields(buf)
	local fields = {}
	for row, line in ipairs(vim.api.nvim_buf_get_lines(buf, 0, -1, false)) do
		if line == "" then
			break
		end

		local name, value = line:match("^([^:%s]+):(.*)$")
		local last = fields[#fields]
		if name then
			table.insert(fields, { name = name:lower(), row = row, value = vim.trim(value) })
		elseif last and line:match("^[ \t]") then
			last.value = vim.trim(last.value .. " " .. vim.trim(line))
		end
	end

	return fields
end

return M
