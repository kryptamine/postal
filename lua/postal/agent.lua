-- A chat with an ACP agent about one mail buffer, a Draft, a Signature or a
-- Message opened to read: its pane, its session, and the one write the agent
-- is allowed, to a Draft's body or a Signature, reviewed hunk by hunk (ADR
-- 0018, 0019, 0020).
local acp = require("postal.acp")
local body = require("postal.body")
local config = require("postal.config")
local kinds = require("postal.kinds")
local pane = require("postal.pane")
local quote = require("postal.quote")
local requirements = require("postal.requirements")
local review = require("postal.review")
local tools = require("postal.tools")

local M = {}

local chats = {}

local MCP = vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":p:h") .. "/mcp.lua"

local Chat = {}
Chat.__index = Chat

--- Asks `postal thread show` for the Thread of the kind's mail; until it
--- answers, `self.thread` is nil, then the text, or false for none.
function Chat:fetch_thread()
	local function done(result)
		if result.code == 0 then
			-- A new mail answers nothing, and its Thread is empty.
			self.thread = result.stdout ~= "" and result.stdout
		else
			self.thread = false
			self.pane:notice("No Thread for the agent: " .. requirements.thread(config.options.cmd, result.stderr))
		end

		self:next()
	end

	local command = { config.options.cmd, "thread", "show", self.kind.thread.flag, self.path }
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
	local first = not self.prompted
	local blocks = {}
	if first then
		self.prompted = true
		table.insert(blocks, { type = "text", text = self.kind.rules })
		vim.list_extend(blocks, self.kind.context(self))
		if self.thread then
			self.pane:dim("context", "Context: " .. self.kind.thread.says)
			table.insert(blocks, self:resource("postal-thread://" .. self.path, self.thread))
		end
	end

	vim.list_extend(blocks, self.kind.turn(self))
	table.insert(blocks, { type = "text", text = instruction })
	self.busy = true
	self.pane:begin_turn()
	self.conn:request("session/prompt", { sessionId = self.session, prompt = blocks }, function(failed, result)
		self.busy = false
		self.pane:end_turn()
		if failed and first then
			-- The rules go with the next prompt instead.
			self.prompted = false
		end

		if failed and failed.code == acp.AUTH_REQUIRED then
			self:sign_in()
			return
		elseif failed then
			self.pane:notice("The agent failed: " .. tostring(failed.message))
		elseif result.stopReason == "cancelled" then
			self.pane:notice("Stopped.")
		elseif result.stopReason ~= "end_turn" then
			self.pane:notice("The agent stopped: " .. tostring(result.stopReason))
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
		self.pane:notice("Sent once the agent's edit is reviewed.")
	elseif self.busy then
		self.pane:notice("Sent once the agent's turn ends.")
	end

	self:next()
end

--- Stops the turn that runs, and drops what waits for it.
function Chat:cancel()
	self.queue = {}
	if self.busy and self.conn then
		self.conn:notify("session/cancel", { sessionId = self.session })
	end
end

function Chat:update(update)
	local kind = update.sessionUpdate
	if kind == "agent_message_chunk" and update.content and update.content.type == "text" then
		self.pane:stream(update.content.text)
	elseif kind == "tool_call" or kind == "tool_call_update" then
		local call = self.calls[update.toolCallId] or {}
		self.calls[update.toolCallId] = vim.tbl_extend("force", call, update)
	end
end

--- Tool call `call`'s title, each absolute path cut to its last two
--- parts, so that a call on a temporary file fits on one line.
local function title(call)
	return (
		tostring(call.title):gsub("/[^%s]+", function(path)
			local tail = path:match("[^/]+/[^/]+$")
			return tail and #tail + 1 < #path and "…/" .. tail or path
		end)
	)
end

function Chat:permit(params, respond)
	local call = vim.tbl_extend("force", self.calls[params.toolCall.toolCallId] or {}, params.toolCall)
	local allowed = tools.asked(self.kind.tools, call)
	-- Never `allow_always`: the agent would stop asking for that tool.
	local wanted = allowed and { "allow_once" } or { "reject_once", "reject_always" }
	for _, option_kind in ipairs(wanted) do
		for _, option in ipairs(params.options or {}) do
			if option.kind == option_kind then
				if not allowed then
					self.pane:dim("denied", "Denied: " .. title(call))
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

--- The body the agent sees: its last write this turn, whatever the review
--- made of it, else the Draft's own.
function Chat:body()
	return self.proposed or body.get(self.mail)
end

--- Opens a review of the agent's body `text`, with the Quote of the one its
--- turn was sent with, against that one; a later write in the same turn
--- replaces the review.
function Chat:propose(text)
	if text ~= "" and not vim.endswith(text, "\n") then
		text = text .. "\n"
	end

	text = quote.kept(self.sent, text)
	self.proposed = text
	if self.review then
		self.review:settle(false, true)
	end

	self.review = review.new(self.mail, self.sent, text, function(summary, undone)
		self:reviewed(summary, undone)
	end)

	if not self.review then
		self.pane:status()
		self.pane:dim("ok", self.kind.write .. " · the edit changes nothing")
		return
	end

	self.pane:dim("ok", self.kind.write .. " · the edit is in the " .. self.kind.noun .. " for review, hunk by hunk")
	self:reviewed(self.review:summary())
end

--- Shows what is left to review; with nothing, sends what waits for it.
function Chat:reviewed(summary, undone)
	self.pane:status(summary)
	if undone then
		self.pane:notice("The agent's edit was undone, and its review ended.")
	end

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

	self.pane:notice(self.name .. " needs you to sign in first. " .. table.concat(how, " "))
	self:stop()
end

--- Ends the agent without the pane saying it ended.
function Chat:stop()
	self.closing = true
	if self.conn then
		self.conn:close()
	end
end

--- Ends a chat whose agent cannot start, saying why.
function Chat:give_up(why)
	self.pane:notice(why)
	vim.notify("postal: " .. why, vim.log.levels.ERROR)
	self.ended = true
end

function Chat:fail_to_start(failed)
	self.pane:notice("The agent failed to start: " .. tostring(failed.message))
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
			args = { "--clean", "--headless", "-l", MCP, vim.v.servername, tostring(self.mail) },
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

--- Starts agent `name` on mail buffer `mail`, taking over the pane and the
--- open review of chat `ended` when given.
local function start(mail, name, ended)
	local kind = kinds[vim.b[mail].postal_kind]
	local self = setmetatable({ mail = mail, kind = kind, name = name, calls = {}, queue = {} }, Chat)
	self.path = vim.api.nvim_buf_get_name(mail)
	-- A review left open holds back the new chat's first prompt.
	self.review = ended and ended.review
	if self.review then
		self.review.changed = function(summary, undone)
			self:reviewed(summary, undone)
		end
	end

	-- The pane outlives an agent that ends, so its keys reach whichever
	-- chat is the mail's now.
	self.pane = ended and ended.pane
		or pane.new(mail, kind, {
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

	self.pane:title(name)
	chats[mail] = self
	local how, missing = requirements.agent(name, config.options.agents[name])
	if not how then
		self:give_up(missing)
		return self
	end

	if how.note then
		self.pane:notice(how.note)
	end

	-- An empty directory: the agent's own tools find nothing on disk, the
	-- Draft's headers and the other Drafts included.
	local cwd = vim.fn.tempname()
	vim.fn.mkdir(cwd, "p")
	local conn, failure = acp.start(how.cmd, cwd, {
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
			self.pane:end_turn()
			if not self.closing then
				self.pane:notice(name .. " ended (" .. code .. "): " .. vim.trim(stderr))
			end
		end,
	})

	if not conn then
		self:give_up(name .. " cannot start: " .. failure)
		return self
	end

	self.conn = conn
	if kind.thread then
		self:fetch_thread()
	else
		self.thread = false
	end

	self:begin(cwd)
	return self
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
