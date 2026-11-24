-- The recipients of a Postal Draft: where one starts under the cursor, and
-- the Correspondents `postal correspondent list` offers for it.
local config = require("postal.config")
local header = require("postal.header")

local M = {}

-- Each Draft's asking: its `job` while it runs, then its `list` of
-- Correspondents, false when Postal could not answer.
local drafts = {}

--- The 0-based byte column where the recipient under the cursor starts, in
--- the `To` or `Cc` header of buffer `buf` or a line folded into it; nil
--- anywhere else. `row` is 1-based and `col` a 0-based byte column.
function M.start(buf, row, col)
	local name, at = header.at(buf, row)
	if not vim.tbl_contains({ "to", "cc" }, name) then
		return nil
	end

	local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
	local line = lines[row]
	for _, piece in ipairs(header.pieces(lines, at - 1)) do
		if piece.row == row - 1 and piece.from <= col and col <= piece.to then
			local start = piece.from
			while start < col and line:sub(start + 1, start + 1):match("%s") do
				start = start + 1
			end

			return start
		end
	end
end

-- Takes the answer of `asking`'s job.
local function take(asking, result)
	asking.job = nil
	if result.code == 0 then
		asking.list = vim.json.decode(result.stdout, { luanil = { object = true } })
	else
		vim.notify_once(vim.trim(result.stderr), vim.log.levels.WARN)
		asking.list = false
	end
end

--- Asks Postal for buffer `buf`'s Correspondents without waiting, so the
--- answer is in by the first recipient typed; `done()` runs once it is.
function M.ask(buf, done)
	local path = vim.api.nvim_buf_get_name(buf)
	local command = { config.options.cmd, "correspondent", "list", "--json", "--draft", path }
	local asking = {}
	drafts[buf] = asking
	local ok, job = pcall(vim.system, command, { text = true }, function(result)
		vim.schedule(function()
			-- `list` may have waited for it already.
			if asking.job then
				take(asking, result)
			end

			if done and vim.api.nvim_buf_is_valid(buf) then
				done()
			end
		end)
	end)

	if ok then
		asking.job = job
	else
		vim.notify_once("postal: " .. tostring(job), vim.log.levels.WARN)
		asking.list = false
	end
end

--- The Correspondents for buffer `buf`'s Draft, best first, each with
--- `address`, `name`, `mailbox`, `account` and `written`; empty when Postal
--- cannot answer, which it says once.
function M.list(buf)
	if not drafts[buf] then
		M.ask(buf)
	end

	local asking = drafts[buf]
	if asking.job then
		take(asking, asking.job:wait())
	end

	return asking.list or {}
end

--- Whether `address` is a Correspondent's of buffer `buf`'s Draft, case
--- aside; nil while Postal has not answered, or could not.
function M.knows(buf, address)
	local list = drafts[buf] and drafts[buf].list
	if not list then
		return nil
	end

	address = address:lower()
	return vim.iter(list):any(function(correspondent)
		return correspondent.address:lower() == address
	end)
end

function M.forget(buf)
	drafts[buf] = nil
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
