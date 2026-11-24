-- Which header of a Postal Draft a line belongs to.
local M = {}

--- The lowercased name of the header line `row` (1-based) of buffer `buf`
--- is in, a folded line counting as its header's, and that header's row;
--- nil in the body or before any header.
function M.at(buf, row)
	local lines = vim.api.nvim_buf_get_lines(buf, 0, row, false)
	for n = 1, row do
		if lines[n] == "" then
			return nil
		end
	end

	local header = row
	while header >= 1 and lines[header]:match("^[ \t]") do
		header = header - 1
	end

	local name = header >= 1 and lines[header]:match("^([^:%s]+):")
	if not name then
		return nil
	end

	return name:lower(), header
end

--- Each header of `buf`, in order: its lowercased `name`, its `row` and
--- `last` row (1-based, the last of its folded lines) and its `value`, a
--- folded line joined on, as Postal reads it.
function M.fields(buf)
	local fields = {}
	for row, line in ipairs(vim.api.nvim_buf_get_lines(buf, 0, -1, false)) do
		if line == "" then
			break
		end

		local name, value = line:match("^([^:%s]+):(.*)$")
		local last = fields[#fields]
		if name then
			table.insert(fields, { name = name:lower(), row = row, last = row, value = vim.trim(value) })
		elseif last and line:match("^[ \t]") then
			last.value = vim.trim(last.value .. " " .. vim.trim(line))
			last.last = row
		end
	end

	return fields
end

--- The pieces of the address header at 0-based `row` of `lines`, cut at
--- each comma outside quotes and `<>`, and at each fold: `{ row, from, to }`
--- (0-based, `to` exclusive), `cut` when a comma or the header ends it.
function M.pieces(lines, row)
	local pieces = {}
	local quoted, angled = false, false
	local at = row
	while lines[at + 1] and (at == row or lines[at + 1]:match("^[ \t]")) do
		local line = lines[at + 1]
		local from = at == row and #line:match("^[^:]*:") or 0
		for n = from + 1, #line do
			local c = line:sub(n, n)
			if c == '"' then
				quoted = not quoted
			elseif c == "<" and not quoted then
				angled = true
			elseif c == ">" and not quoted then
				angled = false
			elseif c == "," and not quoted and not angled then
				table.insert(pieces, { row = at, from = from, to = n - 1, cut = true })
				from = n
			end
		end

		table.insert(pieces, { row = at, from = from, to = #line })
		at = at + 1
	end

	pieces[#pieces].cut = true
	return pieces
end

--- Each recipient of the address header at 0-based `row` of `lines`, its
--- folded pieces joined as Postal reads them: its `text`, and where it
--- starts and ends, `row` and `col` to `end_row` and `end_col`.
function M.recipients(lines, row)
	local found, open = {}, nil
	for _, piece in ipairs(M.pieces(lines, row)) do
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
