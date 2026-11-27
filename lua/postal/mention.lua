-- A warning over a Draft whose own words mention an attachment while no
-- `Attach:` line names a file, the Quote aside.
local config = require("postal.config")
local mail = require("postal.mail")

local M = {}

-- The 0-based row and column of the first mention in `doc`'s own words.
local function first(doc)
	for row = doc.body_start, (doc.quote or #doc.lines) - 1 do
		local lower = doc.lines[row + 1]:lower()
		for _, word in ipairs(config.options.attachment_words) do
			local col = lower:find("%f[%w]" .. vim.pesc(word:lower()))
			if col then
				return row, col - 1
			end
		end
	end
end

--- The warning over Draft buffer `buf`, in a list of one or none.
function M.diagnose(buf)
	local doc = mail.parse(buf)
	local attached = vim.iter(doc.fields):any(function(field)
		return field.name == "attach" and field.value ~= ""
	end)

	local row, col = first(doc)
	local diagnostics = {}
	if row and not attached then
		table.insert(diagnostics, {
			lnum = row,
			col = col,
			message = "mentions an attachment, and no Attach: line names a file",
			severity = vim.diagnostic.severity.WARN,
		})
	end

	return diagnostics
end

return M
