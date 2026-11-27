-- Diagnostics over a Draft's headers left empty: no recipient in `To:` or
-- `Cc:`, which nothing can be sent to, and no `Subject:`.
local address = require("postal.address")
local mail = require("postal.mail")

local M = {}

--- The diagnostics of Draft buffer `buf`'s empty headers as they stand.
function M.diagnose(buf)
	local doc = mail.parse(buf)
	local diagnostics = {}
	local addressed = vim.iter(doc.fields):any(function(field)
		return (field.name == "to" or field.name == "cc") and #address.recipients(doc.lines, field) > 0
	end)

	if not addressed then
		local to = doc:field("to") or doc:field("cc")
		table.insert(diagnostics, {
			lnum = to and to.row or 0,
			col = 0,
			message = "no recipient in To: or Cc:",
			severity = vim.diagnostic.severity.ERROR,
		})
	end

	local subject = doc:field("subject")
	if not subject or subject.value == "" then
		table.insert(diagnostics, {
			lnum = subject and subject.row or 0,
			col = 0,
			message = "the Subject is empty",
			severity = vim.diagnostic.severity.WARN,
		})
	end

	return diagnostics
end

return M
