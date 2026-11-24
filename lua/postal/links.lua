-- The links of a Message opened to read: Postal lays each out as a `[n]`
-- marker in the text and a `[n] URL` line at the end.
local keys = require("postal.keys")

local M = {}

--- Each link of `buf`, by its number: its `url` and the 1-based `row` of
--- its line at the end.
function M.list(buf)
	local found = {}
	for row, line in ipairs(vim.api.nvim_buf_get_lines(buf, 0, -1, false)) do
		local n, url = line:match("^%[(%d+)%] (%S+)$")
		if n then
			found[tonumber(n)] = { url = url, row = row }
		end
	end

	return found
end

-- The URL of the marker under the cursor, nil off one.
local function under()
	local col = vim.api.nvim_win_get_cursor(0)[2] + 1
	for from, n, to in vim.api.nvim_get_current_line():gmatch("()%[(%d+)%]()") do
		if from <= col and col < to then
			local link = M.list(0)[tonumber(n)]
			return link and link.url
		end
	end
end

-- Opens the link of the marker under the cursor, or the URL under it.
local function open()
	local url = under()
	if not url and vim.fn.expand("<cfile>"):match("^%a[%w+.-]*://") then
		url = vim.fn.expand("<cfile>")
	end

	if url then
		vim.ui.open(url)
	else
		vim.notify("postal: no link under the cursor", vim.log.levels.INFO)
	end
end

--- Lists `buf`'s links in the window's location list, each at its line.
function M.show(buf)
	local items = {}
	for n, link in vim.spairs(M.list(buf)) do
		table.insert(items, { bufnr = buf, lnum = link.row, col = 1, text = ("[%d] %s"):format(n, link.url) })
	end

	if #items == 0 then
		vim.notify("postal: the Message has no links", vim.log.levels.INFO)
		return
	end

	vim.fn.setloclist(0, {}, " ", { title = "Links", items = items })
	vim.cmd.lopen()
end

--- Maps the key over Message buffer `buf`, and `:PostalLinks`.
function M.watch(buf)
	keys.map(
		buf,
		keys.bound({ { name = "link", desc = "open the link of the [n] or the URL under the cursor", run = open } })
	)

	vim.api.nvim_buf_create_user_command(buf, "PostalLinks", function()
		M.show(buf)
	end, { desc = "List the Message's links" })
end

return M
