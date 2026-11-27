-- The Quote a mail's body ends in: its `>` lines and the attribution above
-- them, matching `src/render/quoted_text.rs` (ADR 0015).
local M = {}

local LONGEST = 200

-- Matches `src/render/attribution.rs`: a colon at the end, and a date or an
-- address.
local function names(line)
	line = vim.trim(line)
	return vim.endswith(line, ":") and vim.fn.strchars(line) <= LONGEST and line:find("[%d@]") ~= nil
end

--- The 0-based row in `lines` (at or after 0-based `first`) where the
--- trailing Quote starts; nil when `lines` ends in none.
function M.start(lines, first)
	local n, quoted = #lines, false
	while n > first and (vim.startswith(lines[n], ">") or vim.trim(lines[n]) == "") do
		quoted = quoted or vim.startswith(lines[n], ">")
		n = n - 1
	end

	if not quoted then
		return nil
	end

	if n > first and names(lines[n]) then
		n = n - 1
	end

	return n
end

--- `text` cut into the words before its Quote and the Quote; the Quote is
--- "" when `text` ends in none.
function M.cut(text)
	local lines = vim.split(text, "\n", { plain = true })
	if lines[#lines] == "" then
		table.remove(lines)
	end

	local at = M.start(lines, 0)
	if not at then
		return text, ""
	end

	local own = at > 0 and table.concat(lines, "\n", 1, at) .. "\n" or ""
	return own, text:sub(#own + 1)
end

--- The agent's body `written` with the Quote of `sent`, the body its turn
--- was sent with, whatever it made of that Quote.
function M.kept(sent, written)
	local _, quote = M.cut(sent)
	if quote == "" then
		return written
	end

	local own = M.cut(written)
	if not own:find("%S") then
		return own .. quote
	end

	return own:gsub("%s+$", "") .. "\n\n" .. quote:gsub("^%s+", "")
end

return M
