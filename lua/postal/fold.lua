-- The Quote a mail ends in as one closed fold, the user's own words open.
local body = require("postal.body")
local quote = require("postal.quote")

local M = {}

local EXPR = "v:lua.require'postal.fold'.expr(v:lnum)"

--- The 1-based row the Quote of `buf` starts at, nil without one; found
--- again only once the buffer changed, as `foldexpr` asks for each line.
function M.start(buf)
	local tick = vim.b[buf].changedtick
	local seen = vim.b[buf].postal_quote
	if seen and seen.tick == tick then
		return seen.row
	end

	local own, cut = quote.cut(body.get(buf))
	local _, lines = own:gsub("\n", "")
	local row = cut ~= "" and body.start(buf) + lines + 1 or nil
	vim.b[buf].postal_quote = { tick = tick, row = row }
	return row
end

function M.expr(lnum)
	local start = M.start(vim.api.nvim_get_current_buf())
	return start and lnum >= start and "1" or "0"
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
