-- The plugin's highlight groups, each a default link to a stock group so a
-- colorscheme can restyle it.
local M = {}

local LINKS = {
	PostalMe = "Special",
	PostalAgent = "Title",
	PostalRule = "WinSeparator",
	PostalDim = "Comment",
	PostalContext = "Comment",
	PostalOk = "DiagnosticOk",
	PostalDenied = "DiagnosticError",
	PostalNotice = "DiagnosticInfo",
	PostalKey = "Special",
	PostalSection = "Title",
	PostalReviewAdd = "DiffAdd",
	PostalReviewDelete = "DiffDelete",
	PostalReviewConflict = "DiffText",
	PostalAttachSize = "Comment",
	PostalAttachMissing = "DiagnosticError",
}

local function link()
	for name, to in pairs(LINKS) do
		vim.api.nvim_set_hl(0, name, { link = to, default = true })
	end
end

--- Links them now, and again in `augroup` after each `:colorscheme`, which
--- clears them.
function M.setup(augroup)
	link()
	vim.api.nvim_create_autocmd("ColorScheme", { group = augroup, callback = link })
end

return M
