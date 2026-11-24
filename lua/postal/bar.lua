-- The chat pane's winbar and statusline, as 'statusline' items.
local help = require("postal.help")
local icons = require("postal.icons")

local M = {}

local function escape(text)
	return (text:gsub("%%", "%%%%"))
end

-- A key's hint, and its width on screen.
local function hint(key)
	local label = help.label(key.lhs)
	return "%#PostalKey#" .. escape(label) .. "%#PostalDim# " .. key.short,
		vim.fn.strdisplaywidth(label .. " " .. key.short)
end

--- A winbar `width` wide: `summary` of a review unless nil, as many of
--- `keys` with a `short` as fit, in order, and `more` at the right
--- unless nil.
function M.winbar(width, keys, more, summary)
	local right, room = "", width - 2
	if more then
		local wide
		right, wide = hint(more)
		-- The gap, `more` and a space at the right.
		room = room - (2 + wide + 1)
	end

	local bar = { "  " }
	if summary then
		local glyph, group = icons.get("review")
		local lead = glyph .. " " .. summary
		table.insert(bar, "%#" .. group .. "#" .. escape(lead) .. "  ")
		room = room - vim.fn.strdisplaywidth(lead) - 2
	end

	local gap = ""
	for _, key in ipairs(keys) do
		if key.short and key ~= more then
			local text, size = hint(key)
			if #gap + size > room then
				break
			end

			table.insert(bar, gap .. text)
			room, gap = room - #gap - size, "  "
		end
	end

	return table.concat(bar) .. "%=" .. right .. " "
end

--- A statusline naming `agent` and the mail's `subject`, nil for none.
function M.statusline(agent, subject)
	local glyph = icons.get("agent")
	subject = subject ~= "" and subject or "(no subject)"
	return "%#PostalAgent#  " .. escape(glyph .. " " .. agent) .. "%#PostalDim# · " .. escape(subject)
end

return M
