-- Completes a Postal Draft's recipients from its Correspondents and its
-- `Attach:` paths as they are typed (blink.cmp, else Neovim's own menu), and
-- chats with an ACP agent about a Draft or a Message through `:PostalAgent`.
local agent = require("postal.agent")
local attach = require("postal.attach")
local colors = require("postal.colors")
local icons = require("postal.icons")
local recipients = require("postal.recipients")

local M = {}

-- Where Postal keeps its Drafts, whatever `$XDG_DATA_HOME` is.
local DRAFTS = "*/postal/drafts/*.eml"
-- Where Postal writes a Message `E` opens, until the editor exits.
local MESSAGES = "*/postal/messages/*.eml"

-- Registers the source with blink.cmp once, on the first Draft, so blink
-- loads no earlier than it would. False without blink.
local registered = false
local function blink()
	local ok, cmp = pcall(require, "blink.cmp")
	if not ok then
		return false
	end

	if not registered then
		registered = true
		cmp.add_source_provider("postal", { name = "Postal", module = "postal.blink" })
		cmp.add_filetype_source("mail", "postal")
	end

	return true
end

--- Without blink, Neovim's own menu opens as a recipient is typed, fed by
--- `omnifunc` alone, so the body offers nothing.
local function native(buf)
	if vim.fn.exists("+autocomplete") == 0 then
		return
	end

	vim.bo[buf].autocomplete = true
	vim.bo[buf].complete = "o"
	vim.bo[buf].completeopt = "menuone,noselect"
end

local function complete_agent(typed)
	return vim.tbl_filter(function(name)
		return vim.startswith(name, typed)
	end, vim.tbl_keys(agent.agents))
end

local function agent_command(buf, desc)
	vim.api.nvim_buf_create_user_command(buf, "PostalAgent", function(command)
		agent.ask(buf, command.fargs[1])
	end, { nargs = "?", complete = complete_agent, desc = desc })
end

--- `opts.cmd` is the command Postal runs as, `postal` by default;
--- `opts.agents` adds ACP agents by name, `{ cmd = { ... }, npx = "..." }`,
--- `opts.agent` names the one `:PostalAgent` starts, and `opts.nerd_font`
--- draws the pane's icons from a Nerd Font.
function M.setup(opts)
	opts = opts or {}
	if opts.nerd_font ~= nil then
		icons.nerd = opts.nerd_font
	end

	recipients.cmd = opts.cmd or recipients.cmd
	agent.agents = vim.tbl_deep_extend("force", agent.agents, opts.agents or {})
	agent.default = opts.agent or agent.default
	local group = vim.api.nvim_create_augroup("postal", { clear = true })
	colors.setup(group)
	vim.api.nvim_create_autocmd({ "BufReadPost", "BufNewFile" }, {
		group = group,
		pattern = DRAFTS,
		callback = function(event)
			vim.b[event.buf].postal_kind = "draft"
			vim.bo[event.buf].omnifunc = "v:lua.require'postal'.omnifunc"
			recipients.ask(event.buf)
			if not blink() then
				native(event.buf)
			end

			agent_command(event.buf, "Chat with an ACP agent about this Draft")
		end,
	})

	vim.api.nvim_create_autocmd("BufReadPost", {
		group = group,
		pattern = MESSAGES,
		callback = function(event)
			vim.b[event.buf].postal_kind = "message"
			vim.bo[event.buf].readonly = true
			vim.bo[event.buf].modifiable = false
			agent_command(event.buf, "Ask an ACP agent about this Message")
		end,
	})

	vim.api.nvim_create_autocmd("BufWinEnter", {
		group = group,
		pattern = { DRAFTS, MESSAGES },
		callback = function()
			-- Mail is paragraphs on long lines, whatever `wrap` is globally.
			vim.wo[0][0].wrap = true
			vim.wo[0][0].linebreak = true
			vim.wo[0][0].breakindent = true
		end,
	})

	vim.api.nvim_create_autocmd("BufUnload", {
		group = group,
		pattern = { DRAFTS, MESSAGES },
		callback = function(event)
			agent.close(event.buf)
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

-- What the last `findstart` found under the cursor: "attach" or "recipient".
local completing

--- `:h complete-functions` over the recipient or the `Attach:` path under
--- the cursor.
function M.omnifunc(findstart, base)
	if findstart == 1 then
		local row, col = unpack(vim.api.nvim_win_get_cursor(0))
		local start = attach.start(0, row, col)
		completing = start and "attach" or "recipient"
		-- -3 leaves completion without an error message.
		return start or recipients.start(0, row, col) or -3
	end

	if completing == "attach" then
		return { words = attach.list(base), refresh = "always" }
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
