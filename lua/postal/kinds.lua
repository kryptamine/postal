-- What a chat is over each kind of mail buffer, by the name `postal_kind`
-- gives it: the agent's rules and tools, the pane's review keys, the
-- Thread it asks Postal for, and what goes in the first prompt and every one.
local mail = require("postal.mail")
local review = require("postal.review")
local tools = require("postal.tools")

local M = {}

local REVIEWING = "Your last edit is still under the user's review: the text below shows it, "
	.. "the hunks the user has not decided yet included. A new write replaces that review."

-- A turn over a text the agent writes, given by `scheme`: the text as it
-- stands, and the one a write is reviewed against, an open review's
-- undecided hunks rejected.
local function editing(scheme)
	return function(chat)
		chat.proposed = nil
		local text = mail.parse(chat.mail):body()
		chat.sent = chat.review and chat.review:base() or text
		local blocks = chat.review and { { type = "text", text = REVIEWING } } or {}
		table.insert(blocks, chat:resource(scheme .. chat.path, text))
		return blocks
	end
end

M.draft = {
	rules = [[
You are helping the user write the email Draft open in their editor.
- Read and change the Draft's body only through the postal tools `read_draft` and `write_draft`; no other tool reaches the Draft. Its headers are not yours to see or change.
- The quote the body ends in, the `>` lines of the Message the Draft answers and the line naming whom they quote, is not yours: Postal keeps it as it is, whatever a write puts in its place.
- Write the Draft in the language of the correspondence, the Thread's or else the Draft's own, whatever language the user writes to you in, unless they ask for another.
- Never send the Draft: sending is the user's.
- The Thread resource, when there is one, is the whole correspondence, oldest first.
- Each prompt carries the body as it stands, the user's own edits included, then what the user asks.]],
	tools = tools.draft,
	-- The tool whose edit is reviewed, and what it is an edit to.
	write = "write_draft",
	noun = "Draft",
	review = review.keys,
	-- How `postal thread show` names the Thread, and how the pane does.
	thread = { flag = "--draft", says = "the Thread the Draft answers" },
	context = function()
		return {}
	end,
	turn = editing("postal-draft://"),
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
	context = function()
		return {}
	end,
	turn = editing("postal-signature://"),
}

M.message = {
	rules = [[
You are helping the user read the email Message open in their editor: find things in it, explain and analyse it.
- The Message resource is the whole Message, its headers then its body as text, each link's URL listed at the end; the postal tool `read_message` gives it again. No other tool reaches it.
- The Thread resource, when there is one, is the whole correspondence the Message is part of, oldest first, the Message among it.
- It is read-only: you cannot change it, reply to it or send anything. Answer in the chat.]],
	tools = tools.message,
	thread = { flag = "--message", says = "the Thread of the Message" },
	context = function(chat)
		return { chat:resource("postal-message://" .. chat.path, mail.parse(chat.mail):whole()) }
	end,
	turn = function()
		return {}
	end,
}

return M
