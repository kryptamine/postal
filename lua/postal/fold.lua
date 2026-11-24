-- The Quote a mail ends in as one closed fold, the user's own words open.
local body = require("postal.body")
local quote = require("postal.quote")

local M = {}

-- Each buffer's first quoted row, by the `changedtick` it was found at.
local found = {}

--- The 1-based row the Quote of `buf` starts at; nil without one.
function M.start(buf)
	local tick = vim.b[buf].changedtick
	local seen = found[buf]
	if seen and seen.tick == tick then
		return seen.row
	end

	local own, cut = quote.cut(body.get(buf))
	local _, lines = own:gsub("\n", "")
	local row = cut ~= "" and body.start(buf) + lines + 1 or nil
	found[buf] = { tick = tick, row = row }
	return row
end

--- `foldexpr`, asked of each line.
function M.expr(lnum)
	local start = M.start(vim.api.nvim_get_current_buf())
	return start and lnum >= start and "1" or "0"
end

function M.text()
	return ("> %d quoted lines "):format(vim.v.foldend - vim.v.foldstart + 1)
end

--- Folds the current window's Quote, closed the first time it shows `buf`,
--- so a fold the user opened stays open on coming back.
function M.apply(buf)
	local folded = vim.w.postal_folded or {}
	if folded[tostring(buf)] then
		return
	end

	folded[tostring(buf)] = true
	vim.w.postal_folded = folded
	vim.wo[0][0].foldmethod = "expr"
	vim.wo[0][0].foldexpr = "v:lua.require'postal.fold'.expr(v:lnum)"
	vim.wo[0][0].foldtext = "v:lua.require'postal.fold'.text()"
	vim.wo[0][0].foldlevel = 0
	vim.wo[0][0].foldenable = true
end

function M.forget(buf)
	found[buf] = nil
end

return M
