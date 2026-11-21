-- A blink.cmp source of Correspondents for the recipients of a Postal Draft.
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
	local start = recipients.start(context.bufnr, row, col)
	if not start then
		callback(none)
		return
	end

	local list = recipients.list(context.bufnr)
	local kind = require("blink.cmp.types").CompletionItemKind.Text
	local items = {}
	for n, correspondent in ipairs(list) do
		table.insert(items, {
			label = correspondent.mailbox,
			filterText = (correspondent.name or "") .. " " .. correspondent.address,
			labelDetails = { description = correspondent.account },
			kind = kind,
			-- Postal's rank, which the fuzzy score would otherwise drown.
			score_offset = math.floor(10 * (#list - n) / #list),
			textEdit = {
				newText = correspondent.mailbox,
				range = {
					start = { line = row - 1, character = start },
					["end"] = { line = row - 1, character = col },
				},
			},
		})
	end

	callback({ items = items, is_incomplete_forward = false, is_incomplete_backward = false })
end

return Source
