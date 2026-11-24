-- Diagnostics over a Draft's `To:` and `Cc:`: a recipient Postal would
-- refuse to send to, and an address no mail of the accounts holds.
local recipients = require("postal.recipients")

local M = {}

local NS = vim.api.nvim_create_namespace("postal-addresses")

-- Close to what a send accepts, not a parse of RFC 5322: a local part, an
-- `@` and a domain with a dot in it.
local ADDRESS = '^[^%s@<>,"]+@[^%s@<>,"]+%.[^%s@<>,".]+$'

-- Each recipient of the header starting at 0-based `row` of `lines`, joined
-- over a fold as Postal reads it: `{ row, col, end_row, end_col, text }`.
local function split(lines, row)
	local found, open = {}, nil
	local quoted, angled = false, false
	-- Adds bytes `from` to `to` of line `at` to the open recipient.
	local function add(at, from, to)
		local text = lines[at + 1]:sub(from + 1, to)
		if not open then
			local lead = #text:match("^%s*")
			if lead == #text then
				return
			end

			open = { row = at, col = from + lead, text = "" }
		end

		open.text = open.text .. " " .. text
		local trail = #text:match("%s*$")
		if trail < #text then
			open.end_row, open.end_col = at, to - trail
		end
	end

	local function close()
		if open then
			open.text = vim.trim(open.text)
			table.insert(found, open)
			open = nil
		end
	end

	local at = row
	while lines[at + 1] and (at == row or lines[at + 1]:match("^[ \t]")) do
		local line = lines[at + 1]
		local from = at == row and #line:match("^[^:]*:") or 0
		for n = from + 1, #line do
			local c = line:sub(n, n)
			if c == '"' then
				quoted = not quoted
			elseif c == "<" and not quoted then
				angled = true
			elseif c == ">" and not quoted then
				angled = false
			elseif c == "," and not quoted and not angled then
				add(at, from, n - 1)
				close()
				from = n
			end
		end

		add(at, from, #line)
		at = at + 1
	end

	close()
	return found
end

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
	for row, line in ipairs(lines) do
		if line == "" then
			break
		end

		local name = line:match("^([^:%s]+):")
		if name and vim.tbl_contains({ "to", "cc" }, name:lower()) then
			for _, recipient in ipairs(split(lines, row - 1)) do
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
