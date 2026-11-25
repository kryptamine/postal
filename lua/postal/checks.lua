-- What a Draft is checked for as it is written: its recipients and the
-- attachment its words mention, as diagnostics, and each `Attach:` file's size.
local addresses = require("postal.addresses")
local attach = require("postal.attach")
local mention = require("postal.mention")

local M = {}

M.NS = vim.api.nvim_create_namespace("postal-checks")

-- Each by the `code` its diagnostics carry.
local CHECKS = { address = addresses.diagnose, mention = mention.diagnose }

function M.run(buf)
	local diagnostics = {}
	for code, diagnose in vim.spairs(CHECKS) do
		for _, diagnostic in ipairs(diagnose(buf)) do
			table.insert(diagnostics, vim.tbl_extend("force", diagnostic, { code = code, source = "postal" }))
		end
	end

	vim.diagnostic.set(M.NS, buf, diagnostics)
	attach.mark(buf)
end

--- Keeps Draft buffer `buf` checked in `augroup`; not while a line is
--- typed, a recipient or a path being nothing whole until it ends.
function M.watch(buf, augroup)
	-- Neovim redraws on each DiagnosticChanged: run now, the Draft would be
	-- drawn before its window folds the Quote.
	vim.schedule(function()
		if vim.api.nvim_buf_is_valid(buf) then
			M.run(buf)
		end
	end)
	vim.api.nvim_create_autocmd({ "TextChanged", "InsertLeave", "BufEnter", "FocusGained" }, {
		group = augroup,
		buffer = buf,
		callback = function()
			M.run(buf)
		end,
	})
end

return M
