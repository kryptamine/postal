-- A blink.cmp source of Correspondents for the recipients of a Postal
-- Draft, and of paths for its `Attach:` lines.
local attach = require("postal.attach")
local recipients = require("postal.recipients")

local Source = {}

function Source.new()
	return setmetatable({}, { __index = Source })
end

function Source:enabled()
	return vim.b.postal_kind == "draft"
end

function Source:get_completions(context, callback)
	local none = { items = {}, is_incomplete_forward = false, is_incomplete_backward = false }
	local row, col = context.cursor[1], context.cursor[2]
	local types = require("blink.cmp.types").CompletionItemKind
	local range = function(start)
		return { start = { line = row - 1, character = start }, ["end"] = { line = row - 1, character = col } }
	end

	local path_start = attach.start(context.bufnr, row, col)
	if path_start then
		local typed = vim.api.nvim_buf_get_lines(context.bufnr, row - 1, row, false)[1]:sub(path_start + 1, col)
		local items = vim.tbl_map(function(path)
			return { label = path, kind = types.File, textEdit = { newText = path, range = range(path_start) } }
		end, attach.list(typed))
		-- Each `/` typed opens another directory, so blink asks on every key.
		callback({ items = items, is_incomplete_forward = true, is_incomplete_backward = true })
		return
	end

	local start = recipients.start(context.bufnr, row, col)
	if not start then
		callback(none)
		return
	end

	local list = recipients.list(context.bufnr)
	local kind = types.Text
	local items = {}
	for n, correspondent in ipairs(list) do
		table.insert(items, {
			label = correspondent.mailbox,
			filterText = (correspondent.name or "") .. " " .. correspondent.address,
			labelDetails = { description = correspondent.account },
			kind = kind,
			-- Postal's rank, which the fuzzy score would otherwise drown.
			score_offset = math.floor(10 * (#list - n) / #list),
			textEdit = { newText = correspondent.mailbox, range = range(start) },
		})
	end

	callback({ items = items, is_incomplete_forward = false, is_incomplete_backward = false })
end

return Source
