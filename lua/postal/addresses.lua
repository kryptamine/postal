-- Diagnostics over a Draft's `To:` and `Cc:`: a recipient Postal would
-- refuse to send to, and an address no mail of the accounts holds.
local address = require("postal.address")
local mail = require("postal.mail")
local recipients = require("postal.recipients")

local M = {}

-- Close to what a send accepts, not a parse of RFC 5322: a local part, an
-- `@` and a domain with a dot in it.
local ADDRESS = '^[^%s@<>,"]+@[^%s@<>,"]+%.[^%s@<>,".]+$'

-- What is wrong with recipient `text`, and how much; nil when nothing.
local function problem(buf, text)
	local _, quotes = text:gsub('"', "")
	if quotes % 2 == 1 then
		return "a quote is not closed", vim.diagnostic.severity.ERROR
	end

	local open, close = text:find("<", 1, true), text:find(">", 1, true)
	if open and not close or close and not open then
		return "a `<` and its `>` do not pair", vim.diagnostic.severity.ERROR
	end

	local address = text:match("<%s*(.-)%s*>") or text
	if not address:match(ADDRESS) then
		return ("`%s` is not an address"):format(address), vim.diagnostic.severity.ERROR
	end

	if recipients.knows(buf, address) == false then
		return ("no mail of the accounts is from or to %s"):format(address), vim.diagnostic.severity.INFO
	end
end

--- The diagnostics of Draft buffer `buf`'s headers as they stand.
function M.diagnose(buf)
	local doc = mail.parse(buf)
	local diagnostics = {}
	for _, field in ipairs(doc.fields) do
		if field.name == "to" or field.name == "cc" then
			for _, recipient in ipairs(address.recipients(doc.lines, field)) do
				local message, severity = problem(buf, recipient.text)
				if message then
					table.insert(diagnostics, {
						lnum = recipient.row,
						col = recipient.col,
						end_lnum = recipient.end_row,
						end_col = recipient.end_col,
						message = message,
						severity = severity,
					})
				end
			end
		end
	end

	return diagnostics
end

return M
