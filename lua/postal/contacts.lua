-- `:PostalContacts`: a Correspondent picked through `vim.ui.select`, added
-- to the `To:` or `Cc:` under the cursor, else to `To:`.
local header = require("postal.header")
local recipients = require("postal.recipients")

local M = {}

-- The 0-based row of the last line of the header `name` of `buf`, a folded
-- line counting as its; nil without one.
local function last_row(buf, name)
	local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
	for row, line in ipairs(lines) do
		if line == "" then
			return nil
		end

		if line:lower():match("^" .. name .. ":") then
			while lines[row + 1] and lines[row + 1]:match("^[ \t]") do
				row = row + 1
			end

			return row - 1
		end
	end
end

--- Adds `mailbox` to header `name` of `buf` after its recipients.
function M.add(buf, name, mailbox)
	local row = last_row(buf, name)
	if not row then
		vim.notify("postal: the Draft has no " .. name .. ": header", vim.log.levels.WARN)
		return
	end

	local line = vim.api.nvim_buf_get_lines(buf, row, row + 1, false)[1]
	local bare = line:gsub("[,%s]+$", "")
	local empty = bare:lower() == name .. ":"
	vim.api.nvim_buf_set_lines(buf, row, row + 1, false, { bare .. (empty and " " or ", ") .. mailbox })
end

function M.pick(buf)
	local name = header.at(buf, vim.api.nvim_win_get_cursor(0)[1])
	name = (name == "cc" or name == "to") and name or "to"
	vim.ui.select(recipients.list(buf), {
		prompt = "Add to " .. name:gsub("^%l", string.upper) .. ":",
		format_item = function(correspondent)
			return correspondent.mailbox .. "  " .. correspondent.account
		end,
	}, function(correspondent)
		if correspondent then
			M.add(buf, name, correspondent.mailbox)
		end
	end)
end

return M
