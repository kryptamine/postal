-- The pane's looks: a glyph and the group it is drawn in. The glyph is a
-- Nerd Font one when `nerd` is on, else one any font has.
local M = { nerd = vim.g.have_nerd_font == true }

local LOOKS = {
	me = { "\u{f007}", "▍", "PostalMe" },
	agent = { "\u{f06a9}", "▍", "PostalAgent" },
	ok = { "\u{f00c}", "✓", "PostalOk" },
	denied = { "\u{f05e}", "✗", "PostalDenied" },
	context = { "\u{f0e0}", "≡", "PostalContext" },
	notice = { "\u{f05a}", "▎", "PostalNotice" },
	review = { "\u{eae1}", "±", "PostalAgent" },
	keys = { "\u{f030c}", "?", "PostalSection" },
}

--- The glyph of `name`, and its group.
function M.get(name)
	local look = LOOKS[name]
	return look[M.nerd and 1 or 2], look[3]
end

return M
