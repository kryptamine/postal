-- Completes a Postal Draft's recipients from its Correspondents and its
-- `Attach:` paths as they are typed (blink.cmp, else Neovim's own menu), and
-- chats with an ACP agent about a Draft, a Signature or a Message through
-- `:PostalAgent`.
local addresses = require("postal.addresses")
local agent = require("postal.agent")
local attach = require("postal.attach")
local colors = require("postal.colors")
local contacts = require("postal.contacts")
local fold = require("postal.fold")
local icons = require("postal.icons")
local keys = require("postal.keys")
local layout = require("postal.layout")
local links = require("postal.links")
local mention = require("postal.mention")
local prompts = require("postal.prompts")
local recipients = require("postal.recipients")

local M = {}

-- Where Postal keeps its Drafts, whatever `$XDG_DATA_HOME` is.
local DRAFTS = "*/postal/drafts/*.eml"
-- Where Postal writes a Signature Settings open, until the editor exits.
local SIGNATURES = "*/postal/signatures/*.txt"
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

-- `:PostalAgent [agent] [prompt]`, each by its name in setup().
local function agent_command(buf, desc)
	local function named()
		return prompts.over(vim.b[buf].postal_kind)
	end

	local function complete(typed)
		local names = vim.list_extend(vim.tbl_keys(agent.agents), vim.tbl_keys(named()))
		return vim.tbl_filter(function(name)
			return vim.startswith(name, typed)
		end, names)
	end

	vim.api.nvim_buf_create_user_command(buf, "PostalAgent", function(command)
		local texts, name, text = named(), nil, nil
		for _, word in ipairs(command.fargs) do
			if agent.agents[word] and not name and not text then
				name = word
			elseif texts[word] and not text then
				text = texts[word]
			else
				vim.notify("postal: no agent or prompt " .. word .. " in setup()", vim.log.levels.ERROR)
				return
			end
		end

		agent.ask(buf, name, text)
	end, { nargs = "*", complete = complete, desc = desc })
end

--- `opts` as the Configuration of nvim/README.md lists them.
function M.setup(opts)
	opts = opts or {}
	keys.set(opts.keys)
	prompts.set(opts.prompts)
	if opts.nerd_font ~= nil then
		icons.nerd = opts.nerd_font
	end

	recipients.cmd = opts.cmd or recipients.cmd
	mention.set(opts.attachment_words)
	layout.spell = opts.spell or false
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
			recipients.ask(event.buf, function()
				addresses.check(event.buf)
			end)
			if not blink() then
				native(event.buf)
			end

			agent_command(event.buf, "Chat with an ACP agent about this Draft")
			vim.api.nvim_buf_create_user_command(event.buf, "PostalContacts", function()
				contacts.pick(event.buf)
			end, { desc = "Add a Correspondent to the Draft's To: or Cc:" })
			attach.watch(event.buf, group)
			addresses.watch(event.buf, group)
			mention.watch(event.buf, group)
		end,
	})

	vim.api.nvim_create_autocmd({ "BufReadPost", "BufNewFile" }, {
		group = group,
		pattern = SIGNATURES,
		callback = function(event)
			vim.b[event.buf].postal_kind = "signature"
			agent_command(event.buf, "Chat with an ACP agent about this Signature")
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
			links.watch(event.buf)
		end,
	})

	vim.api.nvim_create_autocmd("BufWinEnter", {
		group = group,
		pattern = { DRAFTS, MESSAGES, SIGNATURES },
		callback = function(event)
			layout.apply(event.buf)
		end,
	})

	vim.api.nvim_create_autocmd("BufUnload", {
		group = group,
		pattern = { DRAFTS, MESSAGES, SIGNATURES },
		callback = function(event)
			agent.close(event.buf)
			fold.forget(event.buf)
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
