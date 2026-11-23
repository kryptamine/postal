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

return M
