-- Completes a Postal Draft's recipients from its Correspondents and its
-- `Attach:` paths as they are typed (blink.cmp, else Neovim's own menu), and
-- chats with an ACP agent about a Draft, a Signature or a Message through
-- `:PostalAgent`.
local agent = require("postal.agent")
local attach = require("postal.attach")
local checks = require("postal.checks")
local colors = require("postal.colors")
local config = require("postal.config")
local contacts = require("postal.contacts")
local keys = require("postal.keys")
local layout = require("postal.layout")
local links = require("postal.links")
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

-- `:PostalAgent [agent] [prompt]` over buffer `buf` of `kind`, each by its
-- name in setup().
local function agent_command(buf, kind)
	local texts = function()
		return prompts.over(kind.edits)
	end

	local function complete(typed)
		local names = vim.list_extend(vim.tbl_keys(config.options.agents), vim.tbl_keys(texts()))
		return vim.tbl_filter(function(name)
			return vim.startswith(name, typed)
		end, names)
	end

	vim.api.nvim_buf_create_user_command(buf, "PostalAgent", function(command)
		local words = vim.deepcopy(command.fargs)
		local name = config.options.agents[words[1]] and table.remove(words, 1) or nil
		local text = words[1] and texts()[words[1]]
		if #words > 1 or (words[1] and not text) then
			vim.notify("postal: no agent or prompt " .. words[1] .. " in setup()", vim.log.levels.ERROR)
			return
		end

		agent.ask(buf, name, text)
	end, { nargs = "*", complete = complete, desc = kind.desc })
end

-- Each kind of mail buffer by its `postal_kind`: the files it is, whether
-- the user edits it and whether it ends in a Quote, and what it sets up.
local KINDS = {
	draft = {
		pattern = DRAFTS,
		events = { "BufReadPost", "BufNewFile" },
		edits = true,
		quoted = true,
		desc = "Chat with an ACP agent about this Draft",
		setup = function(buf, group)
			vim.bo[buf].omnifunc = "v:lua.require'postal'.omnifunc"
			recipients.ask(buf, function()
				checks.run(buf)
			end)
			if not blink() then
				native(buf)
			end

			vim.api.nvim_buf_create_user_command(buf, "PostalContacts", function()
				contacts.pick(buf)
			end, { desc = "Add a Correspondent to the Draft's To: or Cc:" })
			attach.map(buf)
			checks.watch(buf, group)
		end,
		unload = recipients.forget,
	},
	signature = {
		pattern = SIGNATURES,
		events = { "BufReadPost", "BufNewFile" },
		edits = true,
		quoted = false,
		desc = "Chat with an ACP agent about this Signature",
	},
	message = {
		pattern = MESSAGES,
		events = { "BufReadPost" },
		edits = false,
		quoted = true,
		desc = "Ask an ACP agent about this Message",
		setup = function(buf)
			vim.bo[buf].readonly = true
			vim.bo[buf].modifiable = false
			links.watch(buf)
		end,
	},
}

--- `opts` as the Configuration of nvim/README.md lists them.
function M.setup(opts)
	opts = opts or {}
	config.set(opts)
	keys.set(opts.keys)
	prompts.set(opts.prompts)
	local group = vim.api.nvim_create_augroup("postal", { clear = true })
	colors.setup(group)
	for name, kind in pairs(KINDS) do
		vim.api.nvim_create_autocmd(kind.events, {
			group = group,
			pattern = kind.pattern,
			callback = function(event)
				vim.b[event.buf].postal_kind = name
				agent_command(event.buf, kind)
				if kind.setup then
					kind.setup(event.buf, group)
				end
			end,
		})

		vim.api.nvim_create_autocmd("BufWinEnter", {
			group = group,
			pattern = kind.pattern,
			callback = function(event)
				layout.apply(event.buf, kind)
			end,
		})

		vim.api.nvim_create_autocmd("BufUnload", {
			group = group,
			pattern = kind.pattern,
			callback = function(event)
				agent.close(event.buf)
				if kind.unload then
					kind.unload(event.buf)
				end
			end,
		})
	end
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
