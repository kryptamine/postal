-- The Quote a mail ends in as one closed fold, the user's own words open.
local mail = require("postal.mail")

local M = {}

local EXPR = "v:lua.require'postal.fold'.expr(v:lnum)"

function M.expr(lnum)
	-- `lnum` counts from 1, the Quote's row from 0.
	local quote = mail.parse(vim.api.nvim_get_current_buf()).quote
	return quote and lnum > quote and "1" or "0"
end

function M.text()
	return ("> %d quoted lines "):format(vim.v.foldend - vim.v.foldstart + 1)
end

--- Folds the current window's Quote of `buf`, closed; a window that folds
--- it already keeps it as the user left it.
function M.apply(buf)
	if vim.wo[0][0].foldexpr == EXPR then
		return
	end

	vim.wo[0][0].foldmethod = "expr"
	vim.wo[0][0].foldexpr = EXPR
	vim.wo[0][0].foldtext = "v:lua.require'postal.fold'.text()"
	vim.wo[0][0].foldlevel = 0
	vim.wo[0][0].foldenable = true
end

return M
