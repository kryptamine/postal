-- Completes the recipients of a Postal Draft from its Correspondents as
-- they are typed: through blink.cmp when it is installed, else Neovim's own
-- menu, or `<C-x><C-o>` before Neovim 0.12.
local recipients = require("postal.recipients")

local M = {}

-- Where Postal keeps its Drafts, whatever `$XDG_DATA_HOME` is.
local DRAFTS = "*/postal/drafts/*.eml"

--- `opts.cmd` is the command Postal runs as, `postal` by default.
function M.setup(opts)
	opts = opts or {}
	recipients.cmd = opts.cmd or recipients.cmd
	local group = vim.api.nvim_create_augroup("postal", { clear = true })
	vim.api.nvim_create_autocmd({ "BufReadPost", "BufNewFile" }, {
		group = group,
		pattern = DRAFTS,
		callback = function(event)
			vim.b[event.buf].postal_draft = true
			vim.bo[event.buf].omnifunc = "v:lua.require'postal'.omnifunc"
			if not M.blink() then
				M.native(event.buf)
			end
		end,
	})

	vim.api.nvim_create_autocmd("BufWipeout", {
		group = group,
		pattern = DRAFTS,
		callback = function(event)
			recipients.forget(event.buf)
		end,
	})
end

-- Registers the source with blink.cmp once, on the first Draft, so blink
-- loads no earlier than it would. False without blink.
local registered = false
function M.blink()
	local ok, blink = pcall(require, "blink.cmp")
	if not ok then
		return false
	end

	if not registered then
		registered = true
		blink.add_source_provider("postal", { name = "Postal", module = "postal.blink" })
		blink.add_filetype_source("mail", "postal")
	end

	return true
end

--- Without blink, Neovim's own menu opens as a recipient is typed, fed by
--- `omnifunc` alone, so the body offers nothing.
function M.native(buf)
	if vim.fn.exists("+autocomplete") == 0 then
		return
	end

	vim.bo[buf].autocomplete = true
	vim.bo[buf].complete = "o"
	vim.bo[buf].completeopt = "menuone,noselect"
end

--- `:h complete-functions` over the recipient under the cursor.
function M.omnifunc(findstart, base)
	if findstart == 1 then
		local row, col = unpack(vim.api.nvim_win_get_cursor(0))
		-- -3 leaves completion without an error message.
		return recipients.start(0, row, col) or -3
	end

	local items = {}
	for _, correspondent in ipairs(recipients.list(0)) do
		if recipients.matches(correspondent, base) then
			table.insert(items, { word = correspondent.mailbox, menu = correspondent.account })
		end
	end

	-- Asked again on each key: Vim would narrow the list by prefix alone,
	-- and `nov` begins no `Petr Novak <…>`.
	return { words = items, refresh = "always" }
end

return M
