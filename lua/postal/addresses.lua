-- Diagnostics over a Draft's `To:` and `Cc:`: a recipient Postal would
-- refuse to send to, and an address no mail of the accounts holds.
local header = require("postal.header")
local recipients = require("postal.recipients")

local M = {}

local NS = vim.api.nvim_create_namespace("postal-addresses")

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

--- Sets the diagnostics of Draft buffer `buf` from its headers as they stand.
function M.check(buf)
	local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
	local diagnostics = {}
	for _, field in ipairs(header.fields(buf)) do
		if field.name == "to" or field.name == "cc" then
			for _, recipient in ipairs(header.recipients(lines, field.row - 1)) do
				local message, severity = problem(buf, recipient.text)
				if message then
					table.insert(diagnostics, {
						lnum = recipient.row,
						col = recipient.col,
						end_lnum = recipient.end_row,
						end_col = recipient.end_col,
						message = message,
						severity = severity,
						source = "postal",
					})
				end
			end
		end
	end

	vim.diagnostic.set(NS, buf, diagnostics)
end

--- Keeps Draft buffer `buf` checked in `augroup`, but not while a recipient
--- is typed, which is no address until whole.
function M.watch(buf, augroup)
	M.check(buf)
	vim.api.nvim_create_autocmd({ "TextChanged", "InsertLeave" }, {
		group = augroup,
		buffer = buf,
		callback = function()
			M.check(buf)
		end,
	})
end

return M
