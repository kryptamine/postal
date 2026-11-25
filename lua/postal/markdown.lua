-- A Draft whose Account sends Markdown, as `$POSTAL_COMPOSE_FORMAT` says:
-- the text the user writes is parsed as Markdown, for Neovim's highlights
-- and render-markdown.nvim, which takes any filetype with `markdown` in it.
local body = require("postal.body")

local M = {}

local FILETYPE = "mail.markdown"
-- Postal keeps what follows either as written, out of Markdown.
local SEPARATOR = "-- "
local FORWARDED = "---------- Forwarded message ---------"

--- Whether Postal opened this editor on a Draft it sends as Markdown.
function M.on()
	return vim.env.POSTAL_COMPOSE_FORMAT == "markdown"
end

--- The 0-based rows `[first, last)` of buffer `buf` Postal converts, as
--- `compose::markdown` cuts them: before a Forward's original, else the
--- trailing Quote and the line naming whom it quotes, then the last `-- `.
function M.own(buf)
	local first = body.start(buf)
	local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
	local last
	for row = first + 1, #lines do
		if lines[row] == FORWARDED then
			last = row - 1
			break
		end
	end

	if not last then
		local quoted
		last = #lines
		while last > first and (lines[last]:match("^>") or vim.trim(lines[last]) == "") do
			quoted = quoted or lines[last]:match("^>")
			last = last - 1
		end

		if not quoted then
			last = #lines
		elseif last > first and lines[last]:match(":%s*$") then
			last = last - 1
		end
	end

	for row = last, first + 1, -1 do
		if lines[row] == SEPARATOR then
			return first, row - 1
		end
	end

	return first, last
end

--- Parses only the rows Postal converts, so a header or the signature's
--- `-- ` never reads as Markdown.
local function regions(buf)
	local parser = vim.treesitter.get_parser(buf, "markdown", { error = false })
	if not parser then
		return
	end

	local first, last = M.own(buf)
	local from, to = vim.api.nvim_buf_get_offset(buf, first), vim.api.nvim_buf_get_offset(buf, last)
	parser:set_included_regions({ { { first, 0, from, last, 0, to } } })
end

--- Makes Draft buffer `buf` a Markdown one when Postal sends it so, before
--- anything else sets it up: the filetype runs the markdown ftplugin, whose
--- options would replace the plugin's own.
function M.attach(buf, group)
	if not M.on() or not vim.treesitter.language.add("markdown") then
		return
	end

	vim.treesitter.language.register("markdown", FILETYPE)
	vim.bo[buf].filetype = FILETYPE
	vim.treesitter.start(buf, "markdown")
	-- Treesitter turns syntax off; the mail syntax still colours the headers.
	vim.bo[buf].syntax = "mail"
	regions(buf)
	vim.api.nvim_create_autocmd({ "TextChanged", "TextChangedI" }, {
		group = group,
		buffer = buf,
		callback = function()
			regions(buf)
		end,
	})
end

--- Whether buffer `buf` is a Draft `attach` made Markdown.
function M.is(buf)
	return vim.bo[buf].filetype == FILETYPE
end

return M
