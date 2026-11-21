-- A chat with an ACP agent about one Draft buffer: its pane, its session,
-- and the one write the agent is allowed, to the Draft's body through
-- Postal's MCP server, reviewed hunk by hunk (ADR 0018, 0019, 0020).
local acp = require("postal.acp")
local body = require("postal.body")
local pane = require("postal.pane")
local recipients = require("postal.recipients")
local requirements = require("postal.requirements")
local review = require("postal.review")

local M = {}

--- Each agent by name, `{ cmd = { ... }, npx = "..." }`, and the one a chat
--- starts with; `npx` names the package npx runs when `cmd` is not on PATH.
M.agents = {
	claude = { cmd = { "claude-agent-acp" }, npx = "@agentclientprotocol/claude-agent-acp" },
}
M.default = "claude"

local RULES = [[
You are helping the user write the email Draft open in their editor.
- Read and change the Draft's body only through the postal tools `read_draft` and `write_draft`; no other tool reaches the Draft. Its headers are not yours to see or change.
- Keep the quote, the `>` lines of the Message the Draft answers, as it is.
- Never send the Draft: sending is the user's.
- The Thread resource, when there is one, is the whole correspondence, oldest first.
- Each prompt carries the body as it stands, the user's own edits included, then what the user asks.]]

local chats = {}

-- The tools of Postal's MCP server, as Claude names them and as the server does.
local TOOLS = {
	mcp__postal__read_draft = true,
	mcp__postal__write_draft = true,
	read_draft = true,
	write_draft = true,
}

local MCP = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":p:h") .. "/mcp.lua"

local Chat = {}
Chat.__index = Chat

--- Asks `postal thread show` for the Thread the Draft answers; until it
--- answers, `self.thread` is nil, then the text or false.
function Chat:fetch_thread()
	local function done(result)
		if result.code == 0 then
			self.thread = result.stdout
		else
			self.thread = false
			self.pane:say("No Thread for the agent: " .. requirements.thread(recipients.cmd, result.stderr))
		end

		self:next()
	end

	local command = { recipients.cmd, "thread", "show", "--draft", self.path }
	local ok, failure = pcall(vim.system, command, { text = true }, function(result)
		vim.schedule(function()
			done(result)
		end)
	end)

	if not ok then
		done({ code = -1, stderr = tostring(failure) })
	end
end

--- A resource block, or the same text plainly to an agent that takes none.
function Chat:resource(uri, text)
	if self.embedded then
		return { type = "resource", resource = { uri = uri, mimeType = "text/plain", text = text } }
	end

	return { type = "text", text = uri .. "\n\n" .. text }
end

function Chat:prompt(instruction)
	self.pane:say("You: " .. instruction)
	local first = not self.prompted
	local blocks = {}
	if first then
		self.prompted = true
		table.insert(blocks, { type = "text", text = RULES })
		if self.thread then
			table.insert(blocks, self:resource("postal-thread://" .. self.path, self.thread))
		end
	end

	self.sent = body.get(self.draft)
	self.proposed = nil
	table.insert(blocks, self:resource("postal-draft://" .. self.path, self.sent))
	table.insert(blocks, { type = "text", text = instruction })
	self.busy = true
	self.conn:request("session/prompt", { sessionId = self.session, prompt = blocks }, function(failed, result)
		self.busy = false
		if failed and first then
			-- The rules go with the next prompt instead.
			self.prompted = false
		end

		if failed and failed.code == acp.AUTH_REQUIRED then
			self:sign_in()
			return
		elseif failed then
			self.pane:say("The agent failed: " .. tostring(failed.message))
		elseif result.stopReason ~= "end_turn" then
			self.pane:say("The agent stopped: " .. tostring(result.stopReason))
		end

		self:next()
	end)
end

--- Prompts with the first instruction waiting, once the session is up, no
--- turn runs and no edit of the agent's waits for review.
function Chat:next()
	if self.session and self.thread ~= nil and not self.busy and not self.review and #self.queue > 0 then
		self:prompt(table.remove(self.queue, 1))
	end
end

--- Sends `instruction` when the agent is free, the body as it is then.
function Chat:send(instruction)
	table.insert(self.queue, instruction)
	if self.review then
		self.pane:say("Sent once the agent's edit is reviewed: " .. instruction)
	end

	self:next()
end

function Chat:update(update)
	local kind = update.sessionUpdate
	if kind == "agent_message_chunk" and update.content and update.content.type == "text" then
		self.pane:stream(self.name, update.content.text)
	elseif kind == "tool_call" or kind == "tool_call_update" then
		local call = self.calls[update.toolCallId] or {}
		self.calls[update.toolCallId] = vim.tbl_extend("force", call, update)
	end
end

--- Whether a tool call is one of Postal's MCP server, known by its exact
--- name: a title alone could be a shell command that merely says it.
local function postal_tool(call)
	local meta = call._meta and call._meta.claudeCode
	return TOOLS[meta and meta.toolName or call.title] == true
end

function Chat:permit(params, respond)
	local call = vim.tbl_extend("force", self.calls[params.toolCall.toolCallId] or {}, params.toolCall)
	local allowed = postal_tool(call)
	-- Never `allow_always`: the agent would stop asking for that tool.
	local wanted = allowed and { "allow_once" } or { "reject_once", "reject_always" }
	for _, kind in ipairs(wanted) do
		for _, option in ipairs(params.options or {}) do
			if option.kind == kind then
				if not allowed then
					self.pane:say("Denied: " .. tostring(call.title))
				end

				respond({ outcome = { outcome = "selected", optionId = option.optionId } })
				return
			end
		end
	end

	respond({ outcome = { outcome = "cancelled" } })
end

function Chat:request(method, params, respond)
	if method == "session/request_permission" then
		self:permit(params, respond)
	else
		respond(nil, { code = acp.NOT_FOUND, message = method .. " is not offered" })
	end
end

--- Runs tool `name` of Postal's MCP server; `{ text = ..., failed = ... }`.
function Chat:tool(name, args)
	if name == "read_draft" then
		-- The agent's last write this turn, whatever the review made of it.
		return { text = self.proposed or body.get(self.draft) }
	elseif name == "write_draft" and type(args.body) == "string" then
		self:propose(args.body)
		return { text = "The new body is in the Draft for the user's review." }
	end

	return { text = "no tool " .. tostring(name) .. " with those arguments", failed = true }
end

--- Opens a review of the agent's body `text` against the one its turn was
--- sent with; a later write in the same turn replaces the review.
function Chat:propose(text)
	if text ~= "" and not vim.endswith(text, "\n") then
		text = text .. "\n"
	end

	self.proposed = text
	if self.review then
		self.review:settle(false, true)
	end

	self.review = review.new(self.draft, self.sent, text, function(summary)
		self:reviewed(summary)
	end)

	if not self.review then
		self.pane:status("")
		self.pane:say("The agent's edit changes nothing.")
		return
	end

	self.pane:say("The agent's edit is in the Draft for review, hunk by hunk.")
	self:reviewed(self.review:summary())
end

--- Shows what is left to review; with nothing, sends what waits for it.
function Chat:reviewed(summary)
	self.pane:status(summary or "")
	if not summary then
		self.review = nil
		self:next()
	end
end

--- Says what the agent's sign-in takes, and ends the chat.
function Chat:sign_in()
	local how = vim.tbl_map(function(method)
		return method.description or method.name
	end, self.auth_methods or {})

	self.pane:say(self.name .. " needs you to sign in first. " .. table.concat(how, " "))
	self:stop()
end

--- Ends the agent without the pane saying it ended.
function Chat:stop()
	self.closing = true
	if self.conn then
		self.conn:close()
	end
end

function Chat:fail_to_start(failed)
	self.pane:say("The agent failed to start: " .. tostring(failed.message))
	self:stop()
end

function Chat:begin(cwd)
	local capabilities = { fs = { readTextFile = false, writeTextFile = false }, terminal = false }
	self.conn:request("initialize", { protocolVersion = 1, clientCapabilities = capabilities }, function(failed, agent)
		if failed then
			self:fail_to_start(failed)
			return
		end

		self.auth_methods = agent.authMethods
		local prompts = agent.agentCapabilities and agent.agentCapabilities.promptCapabilities
		self.embedded = prompts and prompts.embeddedContext == true
		local server = {
			name = "postal",
			command = vim.v.progpath,
			args = { "--clean", "--headless", "-l", MCP, vim.v.servername, tostring(self.draft) },
			env = {},
		}

		self.conn:request("session/new", { cwd = cwd, mcpServers = { server } }, function(refusal, session)
			if refusal and refusal.code == acp.AUTH_REQUIRED then
				self:sign_in()
			elseif refusal then
				self:fail_to_start(refusal)
			else
				self.session = session.sessionId
				self:next()
			end
		end)
	end)
end

--- Starts agent `name` on Draft buffer `draft`, its pane reused when given.
local function start(draft, name, shown)
	local self = setmetatable({ draft = draft, name = name, calls = {}, queue = {} }, Chat)
	self.path = vim.api.nvim_buf_get_name(draft)
	self.pane = shown or pane.new(draft, function()
		M.ask(draft)
	end)

	chats[draft] = self
	local cmd, note = requirements.agent(name, M.agents[name])
	if not cmd then
		self.pane:say(note)
		vim.notify("postal: " .. note, vim.log.levels.ERROR)
		self.ended = true
		return self
	end

	if note then
		self.pane:say(note)
	end

	self:fetch_thread()
	-- An empty directory: the agent's own tools find nothing on disk, the
	-- Draft's headers and the other Drafts included.
	local cwd = vim.fn.tempname()
	vim.fn.mkdir(cwd, "p")
	local conn, failure = acp.start(cmd, cwd, {
		request = function(method, params, respond)
			self:request(method, params, respond)
		end,
		notify = function(method, params)
			if method == "session/update" and params.update then
				self:update(params.update)
			end
		end,
		exit = function(code, stderr)
			self.ended = true
			self.busy = false
			self.queue = {}
			if not self.closing then
				self.pane:say(name .. " ended (" .. code .. "): " .. vim.trim(stderr))
			end
		end,
	})

	if not conn then
		self.pane:say(name .. " cannot start: " .. failure)
		self.ended = true
		return self
	end

	self.conn = conn
	self:begin(cwd)
	return self
end

--- Asks for an instruction and sends it to Draft buffer `draft`'s agent,
--- starting agent `name`, or the default, when none chats about it yet.
function M.ask(draft, name)
	local chat = chats[draft]
	if chat and not chat.ended and name and name ~= chat.name then
		vim.notify("postal: this Draft already chats with " .. chat.name, vim.log.levels.WARN)
		return
	end

	name = name or (chat and chat.name) or M.default
	if not M.agents[name] then
		vim.notify("postal: no agent " .. name .. " in setup()", vim.log.levels.ERROR)
		return
	end

	if not chat or chat.ended then
		chat = start(draft, name, chat and chat.pane)
		if chat.ended then
			return
		end
	end

	chat.pane:show()
	vim.ui.input({ prompt = name .. ": " }, function(instruction)
		if instruction and instruction ~= "" and not chat.ended then
			chat:send(instruction)
		end
	end)
end

--- Answers a call of Postal's MCP server for Draft buffer `draft`.
function M.serve(draft, name, args)
	local chat = chats[draft]
	if not chat or chat.ended then
		return { text = "the chat about this Draft has ended", failed = true }
	end

	return chat:tool(name, args)
end

--- Ends Draft buffer `draft`'s chat and closes its pane.
function M.close(draft)
	local chat = chats[draft]
	chats[draft] = nil
	if not chat then
		return
	end

	chat:stop()
	chat.pane:close()
end

return M
