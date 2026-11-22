-- The file of an `Attach:` line in a Postal Draft: where it starts under
-- the cursor, and the paths it may complete to, as Postal reads them.
local header = require("postal.header")

local M = {}

--- The 0-based byte column where the path of the `Attach:` line under the
--- cursor starts; nil anywhere else. `row` is 1-based.
function M.start(buf, row, col)
	local name, at = header.at(buf, row)
	if name ~= "attach" or at ~= row then
		return nil
	end

	local line = vim.api.nvim_buf_get_lines(buf, row - 1, row, false)[1]
	local start = #line:match("^[^:]*:%s*")
	return start <= col and start or nil
end

--- The files and directories `typed` completes to. A relative path is
--- under the home directory, as Postal reads it, and stays relative.
function M.list(typed)
	local relative = not typed:match("^[~/]")
	local found = vim.fn.getcompletion(relative and "~/" .. typed or typed, "file")
	if relative then
		found = vim.tbl_map(function(path)
			return (path:gsub("^~/", ""))
		end, found)
	end

	return found
end

return M
