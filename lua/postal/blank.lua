-- Diagnostics over a Draft's headers left empty: no recipient in `To:` or
-- `Cc:`, which nothing can be sent to, and no `Subject:`.
local header = require("postal.header")

local M = {}

--- The diagnostics of Draft buffer `buf`'s empty headers as they stand.
function M.diagnose(buf)
	local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
	local fields = header.fields(buf)
	local function find(name)
		return vim.iter(fields):find(function(field)
			return field.name == name
		end)
	end

	local diagnostics = {}
	local addressed = vim.iter(fields):any(function(field)
		return (field.name == "to" or field.name == "cc") and #header.recipients(lines, field.row - 1) > 0
	end)

	if not addressed then
		local to = find("to") or find("cc")
		table.insert(diagnostics, {
			lnum = to and to.row - 1 or 0,
			col = 0,
			message = "no recipient in To: or Cc:",
			severity = vim.diagnostic.severity.ERROR,
		})
	end

	local subject = find("subject")
	if not subject or subject.value == "" then
		table.insert(diagnostics, {
			lnum = subject and subject.row - 1 or 0,
			col = 0,
			message = "the Subject is empty",
			severity = vim.diagnostic.severity.WARN,
		})
	end

	return diagnostics
end

return M
