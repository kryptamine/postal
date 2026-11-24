-- A warning over a Draft whose own words mention an attachment while no
-- `Attach:` line names a file, the Quote aside.
local body = require("postal.body")
local config = require("postal.config")
local header = require("postal.header")
local quote = require("postal.quote")

local M = {}

local NS = vim.api.nvim_create_namespace("postal-mention")

-- The 0-based row and column of the first mention in `buf`'s own words.
local function first(buf)
	local own = quote.cut(body.get(buf))
	local top = body.start(buf)
	for n, line in ipairs(vim.split(own, "\n", { plain = true })) do
		local lower = line:lower()
		for _, word in ipairs(config.options.attachment_words) do
			local col = lower:find("%f[%w]" .. vim.pesc(word:lower()))
			if col then
				return top + n - 1, col - 1
			end
		end
	end
end

function M.check(buf)
	local attached = vim.iter(header.fields(buf)):any(function(field)
		return field.name == "attach" and field.value ~= ""
	end)

	local row, col = first(buf)
	local diagnostics = {}
	if row and not attached then
		table.insert(diagnostics, {
			lnum = row,
			col = col,
			message = "mentions an attachment, and no Attach: line names a file",
			severity = vim.diagnostic.severity.WARN,
			source = "postal",
		})
	end

	vim.diagnostic.set(NS, buf, diagnostics)
end

--- Keeps Draft buffer `buf` checked in `augroup`.
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
