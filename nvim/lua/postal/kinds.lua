-- What a chat is over each kind of mail buffer, by the name `postal_kind`
-- gives it: the agent's rules and tools, the pane's review keys,
-- and what goes in the first prompt and in every one.
local body = require("postal.body")
local review = require("postal.review")
local tools = require("postal.tools")

local M = {}

M.draft = {
	rules = [[
You are helping the user write the email Draft open in their editor.
- Read and change the Draft's body only through the postal tools `read_draft` and `write_draft`; no other tool reaches the Draft. Its headers are not yours to see or change.
- Keep the quote, the `>` lines of the Message the Draft answers, as it is.
- Never send the Draft: sending is the user's.
- The Thread resource, when there is one, is the whole correspondence, oldest first.
- Each prompt carries the body as it stands, the user's own edits included, then what the user asks.]],
	tools = tools.draft,
	review = review.keys,
	prepare = function(chat)
		chat:fetch_thread()
	end,
	context = function(chat)
		if not chat.thread then
			return {}
		end

		chat.pane:dim("context", "Context: the Thread the Draft answers")
		return { chat:resource("postal-thread://" .. chat.path, chat.thread) }
	end,
	-- The body as it stands, which a write of the agent's is reviewed against.
	turn = function(chat)
		chat.sent = body.get(chat.mail)
		chat.proposed = nil
		return { chat:resource("postal-draft://" .. chat.path, chat.sent) }
	end,
}

M.message = {
	rules = [[
You are helping the user read the email Message open in their editor: find things in it, explain and analyse it.
- The Message resource is the whole Message, its headers then its body as text, each link's URL listed at the end; the postal tool `read_message` gives it again. No other tool reaches it.
- It is read-only: you cannot change it, reply to it or send anything. Answer in the chat.]],
	tools = tools.message,
	prepare = function(chat)
		-- A Message is read alone, without its Thread.
		chat.thread = false
	end,
	context = function(chat)
		return { chat:resource("postal-message://" .. chat.path, body.whole(chat.mail)) }
	end,
	turn = function()
		return {}
	end,
}

return M
