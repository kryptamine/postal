-- A chat with an ACP agent about one Draft buffer: its pane, its session,
-- and the one write the agent is allowed, to the Draft's body (ADR 0018).
local acp = require("postal.acp")
local body = require("postal.body")
local pane = require("postal.pane")
local recipients = require("postal.recipients")

local M = {}

--- Each agent by name, `{ cmd = { ... } }`, and the one a chat starts with.
M.agents = { claude = { cmd = { "claude-agent-acp" } } }
M.default = "claude"

local RULES = [[
You are helping the user write the email Draft open in their editor, the file %s.
- Edit the Draft's body only. Reading that file gives the body alone; writing it replaces the body. The headers are not yours to see or change.
- Keep the quote, the `>` lines of the Message the Draft answers, as it is.
- Never send the Draft: sending is the user's.
- The Thread resource, when there is one, is the whole correspondence, oldest first.
- Each prompt carries the body as it stands, the user's own edits included, then what the user asks.]]

local chats = {}

local function same(a, b)
	if not a or not b then
		return false
	end

	local function real(path)
		return vim.uv.fs_realpath(path) or vim.fs.normalize(path)
	end

	return real(a) == real(b)
end

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
			self.pane:say("No Thread for the agent: " .. vim.trim(result.stderr))
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
		table.insert(blocks, { type = "text", text = RULES:format(self.path) })
		if self.thread then
			table.insert(blocks, self:resource("postal-thread://" .. self.path, self.thread))
		end
	end

	table.insert(blocks, self:resource(vim.uri_from_fname(self.path), body.get(self.draft)))
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

--- Prompts with the first instruction waiting, once the session is up and
--- no turn runs.
function Chat:next()
	if self.session and self.thread ~= nil and not self.busy and #self.queue > 0 then
		self:prompt(table.remove(self.queue, 1))
	end
end

--- Sends `instruction` when the agent is free, the body as it is then.
function Chat:send(instruction)
	table.insert(self.queue, instruction)
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

--- Whether a tool call only edits the Draft.
function Chat:edits_draft(call)
	if call.kind ~= "edit" or not call.locations or #call.locations == 0 then
		return false
	end

	for _, location in ipairs(call.locations) do
		if not same(location.path, self.path) then
			return false
		end
	end

	return true
end

function Chat:permit(params, respond)
	local call = vim.tbl_extend("force", self.calls[params.toolCall.toolCallId] or {}, params.toolCall)
	local allowed = self:edits_draft(call)
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
	local file = method == "fs/read_text_file" or method == "fs/write_text_file"
	if file and not same(params.path, self.path) then
		respond(nil, { code = acp.INVALID_PARAMS, message = "only the Draft " .. self.path .. " is open to the agent" })
	elseif method == "session/request_permission" then
		self:permit(params, respond)
	elseif method == "fs/read_text_file" then
		local lines = vim.split(body.get(self.draft), "\n", { plain = true })
		local first = params.line or 1
		local last = params.limit and first + params.limit - 1 or #lines
		respond({ content = table.concat(vim.list_slice(lines, first, last), "\n") })
	elseif method == "fs/write_text_file" then
		body.set(self.draft, params.content)
		self.pane:say("Changed the body; `u` in the Draft takes it back.")
		respond(vim.empty_dict())
	else
		respond(nil, { code = acp.NOT_FOUND, message = method .. " is not offered" })
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
	local capabilities = { fs = { readTextFile = true, writeTextFile = true }, terminal = false }
	self.conn:request("initialize", { protocolVersion = 1, clientCapabilities = capabilities }, function(failed, agent)
		if failed then
			self:fail_to_start(failed)
			return
		end

		self.auth_methods = agent.authMethods
		local prompts = agent.agentCapabilities and agent.agentCapabilities.promptCapabilities
		self.embedded = prompts and prompts.embeddedContext == true
		self.conn:request("session/new", { cwd = cwd, mcpServers = {} }, function(refusal, session)
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
	self:fetch_thread()
	-- An empty directory: the agent's own tools find nothing on disk, the
	-- Draft's headers and the other Drafts included.
	local cwd = vim.fn.tempname()
	vim.fn.mkdir(cwd, "p")
	local conn, failure = acp.start(M.agents[name].cmd, cwd, {
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
