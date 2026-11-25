-- How a mail buffer is laid out in its window, over what the `mail`
-- ftplugin sets: paragraphs on long lines, wrapped where they are shown.
local config = require("postal.config")
local fold = require("postal.fold")
local markdown = require("postal.markdown")

local M = {}

--- Lays out the current window on mail buffer `buf`, of a `kind` that
--- `edits` or not and is `quoted` or not.
function M.apply(buf, kind)
	vim.wo[0][0].wrap = true
	vim.wo[0][0].linebreak = true
	vim.wo[0][0].breakindent = true
	vim.wo[0][0].spell = config.options.spell and kind.edits
	if kind.quoted then
		fold.apply(buf)
	end

	-- Markdown's marks hide off the cursor's line, as the HTML shows none.
	if markdown.is(buf) then
		vim.wo[0][0].conceallevel = 2
	end

	if kind.edits then
		-- The ftplugin's 72 would break each paragraph typed into hard lines.
		vim.bo[buf].textwidth = 0
		vim.bo[buf].formatoptions = vim.bo[buf].formatoptions:gsub("[tca]", "")
	end
end

return M
