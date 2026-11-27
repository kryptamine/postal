-- One parse of a mail buffer per `changedtick`: folded headers, body start,
-- and the Forward, Quote and Signature cuts (ADR 0015, 0036). Every row in
-- it is 0-based, as the buffer API counts them.
local quote = require("postal.quote")

local M = {}

local SEPARATOR = "-- "
local FORWARDED = "---------- Forwarded message ---------"

local cache = {}
local Mail = {}
Mail.__index = Mail

local function resolve(buf)
	return buf == 0 and vim.api.nvim_get_current_buf() or buf
end

--- Lines as a body's text: empty, or each line ending in a newline.
local function join(lines)
	if #lines == 0 then
		return ""
	end

	return table.concat(lines, "\n") .. "\n"
end

M.join = join

-- The headers, the row the body starts at, and whether a blank line ends
-- the headers; without one the body starts past the last line.
local function parse_headers(lines)
	local fields, current = {}, nil
	for n, line in ipairs(lines) do
		if line == "" then
			return fields, n, true
		end

		local name, value = line:match("^([^:%s]+):(.*)$")
		if name then
			current = { name = name:lower(), row = n - 1, last = n - 1, value = vim.trim(value) }
			table.insert(fields, current)
		elseif current and line:match("^[ \t]") then
			current.value = vim.trim(current.value .. " " .. vim.trim(line))
			current.last = n - 1
		else
			current = nil
		end
	end

	return fields, #lines, false
end

-- The row the trailing Quote starts at, and the end of the rows
-- `compose::markdown` converts: before a Forward's original or the Quote,
-- then before the last `-- `.
local function find_cuts(lines, first)
	local forward
	for row = first, #lines - 1 do
		if lines[row + 1] == FORWARDED then
			forward = row
			break
		end
	end

	local quoted = quote.start(lines, first)
	-- As `quoted_text::cut`, a Quote under no words of the writer's stays in.
	local said = quoted
		and vim.iter(vim.list_slice(lines, first + 1, quoted)):any(function(line)
			return line:find("%S") ~= nil
		end)

	local tail = forward or (said and quoted) or #lines
	for row = tail - 1, first, -1 do
		if lines[row + 1] == SEPARATOR then
			return quoted, row
		end
	end

	return quoted, tail
end

--- The parsed mail in buffer `buf`, reused while `changedtick` and kind hold;
--- shared between callers, so read-only.
function M.parse(buf)
	buf = resolve(buf)
	local tick = vim.b[buf].changedtick
	local kind = vim.b[buf].postal_kind
	local seen = cache[buf]
	if seen and seen.tick == tick and seen.kind == kind then
		return seen.doc
	end

	local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
	local headless = kind == "signature"
	local fields, body_start, body_ended = {}, 0, true
	if not headless then
		fields, body_start, body_ended = parse_headers(lines)
	end

	local quoted, own_end = find_cuts(lines, body_start)
	local doc = setmetatable({
		lines = lines,
		headless = headless,
		fields = fields,
		body_start = body_start,
		body_ended = body_ended,
		quote = quoted,
		own_end = own_end,
	}, Mail)

	cache[buf] = { tick = tick, kind = kind, doc = doc }
	return doc
end

function M.forget(buf)
	cache[resolve(buf)] = nil
end

--- Adds the blank line that ends the headers when there is none, and
--- returns the row the body starts at and whether it added one.
function M.ensure_body(buf)
	local doc = M.parse(buf)
	if doc.body_ended then
		return doc.body_start, false
	end

	local row = doc.body_start
	vim.api.nvim_buf_set_lines(buf, row, row, false, { "" })
	return row + 1, true
end

--- The first header named `name` (any case), or nil.
function Mail:field(name)
	local wanted = name:lower()
	for _, field in ipairs(self.fields) do
		if field.name == wanted then
			return field
		end
	end
end

--- The unfolded value of header `name` (any case), or nil.
function Mail:header(name)
	local field = self:field(name)
	return field and field.value or nil
end

--- The header row `row` belongs to, a folded line counting as its header's;
--- nil outside headers.
function Mail:field_at(row)
	for _, field in ipairs(self.fields) do
		if row >= field.row and row <= field.last then
			return field
		end
	end
end

function Mail:body()
	return join(vim.list_slice(self.lines, self.body_start + 1))
end

function Mail:whole()
	return join(self.lines)
end

return M
