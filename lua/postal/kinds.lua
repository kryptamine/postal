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
- The quote the body ends in, the `>` lines of the Message the Draft answers and the line naming whom they quote, is not yours: Postal keeps it as it is, whatever a write puts in its place.
- Never send the Draft: sending is the user's.
- The Thread resource, when there is one, is the whole correspondence, oldest first.
- Each prompt carries the body as it stands, the user's own edits included, then what the user asks.]],
	tools = tools.draft,
	-- The tool whose edit is reviewed, and what it is an edit to.
	write = "write_draft",
	noun = "Draft",
	review = review.keys,
	prepare = function(chat)
		chat:fetch_thread("--draft")
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

M.signature = {
	rules = [[
You are helping the user write the signature their email account ends its messages with, open in their editor.
- Read and change it only through the postal tools `read_signature` and `write_signature`; no other tool reaches it.
- It is plain text, a few short lines: no HTML, no images, and no `-- ` separator line, which Postal adds itself.
- Each prompt carries the signature as it stands, the user's own edits included, then what the user asks.]],
	tools = tools.signature,
	write = "write_signature",
	noun = "Signature",
	review = review.keys,
	prepare = function(chat)
		-- A Signature answers no Message.
		chat.thread = false
	end,
	context = function()
		return {}
	end,
	turn = function(chat)
		chat.sent = body.get(chat.mail)
		chat.proposed = nil
		return { chat:resource("postal-signature://" .. chat.path, chat.sent) }
	end,
}

M.message = {
	rules = [[
You are helping the user read the email Message open in their editor: find things in it, explain and analyse it.
- The Message resource is the whole Message, its headers then its body as text, each link's URL listed at the end; the postal tool `read_message` gives it again. No other tool reaches it.
- The Thread resource, when there is one, is the whole correspondence the Message is part of, oldest first, the Message among it.
- It is read-only: you cannot change it, reply to it or send anything. Answer in the chat.]],
	tools = tools.message,
	prepare = function(chat)
		chat:fetch_thread("--message")
	end,
	context = function(chat)
		local blocks = { chat:resource("postal-message://" .. chat.path, body.whole(chat.mail)) }
		if chat.thread then
			chat.pane:dim("context", "Context: the Thread of the Message")
			table.insert(blocks, chat:resource("postal-thread://" .. chat.path, chat.thread))
		end

		return blocks
	end,
	turn = function()
		return {}
	end,
}

return M
