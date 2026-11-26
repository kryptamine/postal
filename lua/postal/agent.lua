-- Each mail buffer's chat with an ACP agent, by the buffer: what
-- `:PostalAgent`, Postal's MCP server and `:PostalSend` reach.
local new_chat = require("postal.chat").start
local config = require("postal.config")
local tools = require("postal.tools")

local M = {}

local chats = {}

--- Starts agent `name` on mail buffer `mail` as its chat, taking over the
--- pane of chat `ended` when given.
local function start(mail, name, ended)
	-- Looked up on each key: a pane outlives the chat that opened it.
	chats[mail] = new_chat(mail, name, ended, {
		send = function(text)
			M.send(mail, text)
		end,
		stop = function()
			chats[mail]:cancel()
		end,
		close = function()
			M.close(mail)
		end,
	})

	return chats[mail]
end

--- Opens mail buffer `mail`'s chat, the cursor in its input, starting
--- agent `name`, or the default, when none chats about it yet; sends
--- `text` when given.
function M.ask(mail, name, text)
	local chat = chats[mail]
	if chat and not chat.ended and name and name ~= chat.name then
		vim.notify("postal: this mail already chats with " .. chat.name, vim.log.levels.WARN)
		return
	end

	name = name or (chat and chat.name) or config.options.agent
	if not config.options.agents[name] then
		vim.notify("postal: no agent " .. name .. " in setup()", vim.log.levels.ERROR)
		return
	end

	if not chat or chat.ended then
		chat = start(mail, name, chat)
	end

	chat.pane:show(true)
	if text and not chat.ended then
		chat.pane:post(text)
	end
end

--- Sends `text` to mail buffer `mail`'s agent, starting it again when it
--- has ended.
function M.send(mail, text)
	local chat = chats[mail]
	if chat.ended then
		chat = start(mail, chat.name, chat)
	end

	if not chat.ended then
		chat:send(text)
	end
end

--- The tools Postal's MCP server lists for mail buffer `mail`.
function M.tools(mail)
	local chat = chats[mail]
	return tools.list(chat and chat.kind.tools or {})
end

--- Answers a call of Postal's MCP server for mail buffer `mail`.
function M.serve(mail, name, args)
	local chat = chats[mail]
	if not chat or chat.ended then
		return { content = { { type = "text", text = "the chat about this mail has ended" } }, isError = true }
	end

	return tools.run(chat, name, args)
end

--- Whether an edit of the agent's to mail buffer `mail` waits for review.
function M.reviewing(mail)
	local chat = chats[mail]
	return chat ~= nil and chat.review ~= nil
end

--- Ends mail buffer `mail`'s chat and closes its pane.
function M.close(mail)
	local chat = chats[mail]
	chats[mail] = nil
	if not chat then
		return
	end

	chat:stop()
	chat.pane:close()
end

return M
