-- How a mail buffer is laid out in its window, over what the `mail`
-- ftplugin sets: paragraphs on long lines, wrapped where they are shown.
local fold = require("postal.fold")

local M = {}

--- Whether a Draft or a Signature is spell checked, in the user's `spelllang`.
M.spell = false

--- Lays out the current window on mail buffer `buf`.
function M.apply(buf)
	vim.wo[0][0].wrap = true
	vim.wo[0][0].linebreak = true
	vim.wo[0][0].breakindent = true
	vim.wo[0][0].spell = M.spell and vim.bo[buf].modifiable
	if vim.b[buf].postal_kind ~= "signature" then
		fold.apply(buf)
	end

	if not vim.bo[buf].modifiable then
		return
	end

	-- The ftplugin's 72 would break each paragraph typed into hard lines.
	vim.bo[buf].textwidth = 0
	vim.bo[buf].formatoptions = vim.bo[buf].formatoptions:gsub("[tca]", "")
end

return M
