-- The file of an `Attach:` line in a Postal Draft: where it starts under
-- the cursor, the paths it may complete to and the file it names, as Postal
-- reads them, and each file's size beside its line.
local header = require("postal.header")
local keys = require("postal.keys")

local M = {}

local NS = vim.api.nvim_create_namespace("postal-attach")
-- Postal refuses to send files over this together, in bytes; mirrors
-- `LIMIT` in src/compose/attach.rs.
local LIMIT = 25 * 1024 * 1024

--- The 0-based byte column where the path of the `Attach:` line under the
--- cursor starts; nil anywhere else. `row` is 1-based.
function M.start(buf, row, col)
	local name, at = header.at(buf, row)
	if name ~= "attach" or at ~= row then
		return nil
	end

	local line = vim.api.nvim_buf_get_lines(buf, row - 1, row, false)[1]
	local start = #line:match("^[^:]*:%s*")
	return start <= col and start or nil
end

--- The files and directories `typed` completes to. A relative path is
--- under the home directory, as Postal reads it, and stays relative.
function M.list(typed)
	local relative = not typed:match("^[~/]")
	local found = vim.fn.getcompletion(relative and "~/" .. typed or typed, "file")
	if relative then
		found = vim.tbl_map(function(path)
			return (path:gsub("^~/", ""))
		end, found)
	end

	return found
end

--- The file `written` names, as `Attach::path` in src/compose/attach.rs
--- resolves it: `~/` and a relative path are under the home directory.
--- Nil, and why, without one.
function M.resolve(written)
	local under = written:gsub("^~/", "")
	if vim.startswith(under, "/") then
		return under
	end

	local home = vim.env.HOME
	if not home or home == "" then
		return nil, "no home directory"
	end

	return vim.fs.joinpath(home, under)
end

-- The `Attach:` headers of `buf` naming a file.
local function attached(buf)
	return vim.tbl_filter(function(field)
		return field.name == "attach" and field.value ~= ""
	end, header.fields(buf))
end

local function size(bytes)
	if bytes < 1024 then
		return bytes .. " B"
	elseif bytes < 1024 * 1024 then
		return ("%.0f KB"):format(bytes / 1024)
	end

	return ("%.1f MB"):format(bytes / 1024 / 1024)
end

-- What sits beside an `Attach:` line, its group, and the bytes it adds.
local function state(written)
	local path, why = M.resolve(written)
	local stat = path and vim.uv.fs_stat(path)
	if stat and stat.type == "file" then
		return size(stat.size), "PostalAttachSize", stat.size
	end

	return why or (stat and "not a file" or "no such file"), "PostalAttachMissing", 0
end

--- Marks each `Attach:` line of `buf` with its file's size, or what is
--- wrong with it, and the last one when the files are over Postal's limit.
function M.mark(buf)
	vim.api.nvim_buf_clear_namespace(buf, NS, 0, -1)
	local total, last = 0, nil
	for _, field in ipairs(attached(buf)) do
		local text, group, bytes = state(field.value)
		total, last = total + bytes, field.row - 1
		vim.api.nvim_buf_set_extmark(buf, NS, last, 0, { virt_text = { { text, group } } })
	end

	if total > LIMIT then
		local text = ("%s together, over %d MB"):format(size(total), LIMIT / 1024 / 1024)
		vim.api.nvim_buf_set_extmark(buf, NS, last, 0, { virt_text = { { text, "PostalAttachMissing" } } })
	end
end

--- Edits the file of the `Attach:` line under the cursor; elsewhere, the
--- key does what it does unmapped.
local function open()
	local _, at = header.at(0, vim.api.nvim_win_get_cursor(0)[1])
	local field = vim.iter(attached(0)):find(function(f)
		return f.row == at
	end)

	if not field then
		vim.api.nvim_feedkeys(vim.keycode(keys.lhs.attach), "n", false)
		return
	end

	local path, why = M.resolve(field.value)
	if not path or not vim.uv.fs_stat(path) then
		vim.notify("postal: " .. (why or "no file " .. field.value), vim.log.levels.WARN)
		return
	end

	vim.cmd.edit(vim.fn.fnameescape(path))
end

M.keys = { { name = "attach", desc = "edit the file of the Attach: line", run = open } }

--- Maps the key over Draft buffer `buf`.
function M.map(buf)
	keys.map(buf, keys.bound(M.keys))
end

return M
