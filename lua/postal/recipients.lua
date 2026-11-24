-- The recipients of a Postal Draft: where one starts under the cursor, and
-- the Correspondents `postal correspondent list` offers for it.
local header = require("postal.header")

local M = {}

--- The command Postal runs as; `setup` may point it at a build.
M.cmd = "postal"

-- Each Draft's Correspondents once read, the asking still running, and
-- the Drafts Postal could not answer for.
local lists = {}
local asking = {}
local failed = {}

--- The 0-based byte column where the recipient under the cursor starts, in
--- the `To` or `Cc` header of buffer `buf` or a line folded into it; nil
--- anywhere else. `row` is 1-based and `col` a 0-based byte column.
function M.start(buf, row, col)
	local name, at = header.at(buf, row)
	if not vim.tbl_contains({ "to", "cc" }, name) then
		return nil
	end

	local line = vim.api.nvim_buf_get_lines(buf, row - 1, row, false)[1]
	local before = line:sub(1, col)
	local start = at == row and #name + 1 or 0
	-- A comma inside quotes or angle brackets is part of one recipient.
	local quoted, angled = false, false
	for n = start + 1, #before do
		local c = before:sub(n, n)
		if c == '"' then
			quoted = not quoted
		elseif c == "<" and not quoted then
			angled = true
		elseif c == ">" and not quoted then
			angled = false
		elseif c == "," and not quoted and not angled then
			start = n
		end
	end

	while start < col and before:sub(start + 1, start + 1):match("%s") do
		start = start + 1
	end

	return start
end

-- Takes the answer of buffer `buf`'s asking, which Postal may have refused.
local function take(buf, result)
	asking[buf] = nil
	if result.code == 0 then
		lists[buf] = vim.json.decode(result.stdout, { luanil = { object = true } })
	else
		vim.notify_once(vim.trim(result.stderr), vim.log.levels.WARN)
		lists[buf], failed[buf] = {}, true
	end
end

--- Asks Postal for buffer `buf`'s Correspondents without waiting, so the
--- answer is in by the first recipient typed; `done()` runs once it is.
function M.ask(buf, done)
	local path = vim.api.nvim_buf_get_name(buf)
	local command = { M.cmd, "correspondent", "list", "--json", "--draft", path }
	local job
	local ok, failure = pcall(vim.system, command, { text = true }, function(result)
		vim.schedule(function()
			-- `list` may have waited for it already.
			if asking[buf] == job then
				take(buf, result)
			end

			if done and vim.api.nvim_buf_is_valid(buf) then
				done()
			end
		end)
	end)

	if ok then
		job = failure
		asking[buf] = job
	else
		vim.notify_once("postal: " .. tostring(failure), vim.log.levels.WARN)
		lists[buf], failed[buf] = {}, true
	end
end

--- The Correspondents for buffer `buf`'s Draft, best first, each with
--- `address`, `name`, `mailbox`, `account` and `written`; empty when Postal
--- cannot answer, which it says once.
function M.list(buf)
	if not lists[buf] and not asking[buf] then
		M.ask(buf)
	end

	if asking[buf] then
		take(buf, asking[buf]:wait())
	end

	return lists[buf]
end

--- Whether `address` is a Correspondent's of buffer `buf`'s Draft, case
--- aside; nil while Postal has not answered, or could not.
function M.knows(buf, address)
	if not lists[buf] or failed[buf] then
		return nil
	end

	address = address:lower()
	return vim.iter(lists[buf]):any(function(correspondent)
		return correspondent.address:lower() == address
	end)
end

function M.forget(buf)
	lists[buf] = nil
	asking[buf] = nil
	failed[buf] = nil
end

--- Whether each word of `query` is found in the Correspondent's name or
--- address, case aside.
function M.matches(correspondent, query)
	local haystack = vim.fn.tolower((correspondent.name or "") .. " " .. correspondent.address)
	for word in vim.fn.tolower(query):gmatch("%S+") do
		if not haystack:find(word, 1, true) then
			return false
		end
	end

	return true
end

return M
