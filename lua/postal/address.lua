-- The recipients of an address header (`To:`, `Cc:`) as Postal reads them:
-- cut at each comma outside quotes and `<>`, its folded lines joined.
local M = {}

--- The pieces of header `field` of `lines`, cut at each comma and at each
--- fold: `{ row, from, to }` (0-based, `to` exclusive), `cut` when a comma
--- or the header ends it.
function M.pieces(lines, field)
	local pieces = {}
	local quoted, angled = false, false
	for row = field.row, field.last do
		local line = lines[row + 1]
		local from = row == field.row and #line:match("^[^:]*:") or 0
		for n = from + 1, #line do
			local c = line:sub(n, n)
			if c == '"' then
				quoted = not quoted
			elseif c == "<" and not quoted then
				angled = true
			elseif c == ">" and not quoted then
				angled = false
			elseif c == "," and not quoted and not angled then
				table.insert(pieces, { row = row, from = from, to = n - 1, cut = true })
				from = n
			end
		end

		table.insert(pieces, { row = row, from = from, to = #line })
	end

	pieces[#pieces].cut = true
	return pieces
end

--- Each recipient of header `field` of `lines`, its pieces joined: `text`,
--- and `row`/`col` to `end_row`/`end_col` (0-based).
function M.recipients(lines, field)
	local found, open = {}, nil
	for _, piece in ipairs(M.pieces(lines, field)) do
		local text = lines[piece.row + 1]:sub(piece.from + 1, piece.to)
		local lead, trail = #text:match("^%s*"), #text:match("%s*$")
		if lead < #text then
			open = open or { row = piece.row, col = piece.from + lead, text = "" }
			open.text = vim.trim(open.text .. " " .. vim.trim(text))
			open.end_row, open.end_col = piece.row, piece.to - trail
		end

		if piece.cut and open then
			table.insert(found, open)
			open = nil
		end
	end

	return found
end

return M
