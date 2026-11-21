-- An ACP agent for `make smoke`, run by `nvim -l`: each turn streams a
-- message, asks to run a command and to edit the Draft, writes elsewhere
-- and then upper-cases the body. `locked` wants a sign-in instead.
local log = arg[1]
local locked = log == "locked"

local function record(message)
	if not locked then
		local file = assert(io.open(log, "a"))
		file:write(vim.json.encode(message), "\n")
		file:close()
	end
end

local function send(message)
	message.jsonrpc = "2.0"
	io.stdout:write(vim.json.encode(message), "\n")
	io.stdout:flush()
end

local function receive()
	local line = io.read("l")
	if not line then
		os.exit(0)
	end

	local message = vim.json.decode(line, { luanil = { object = true, array = true } })
	record(message)
	return message
end

local next_id = 0
local function call(method, params)
	next_id = next_id + 1
	send({ id = next_id, method = method, params = params })
	while true do
		local message = receive()
		if message.id == next_id and not message.method then
			return message
		end
	end
end

local function update(session, sessionUpdate)
	send({ method = "session/update", params = { sessionId = session, update = sessionUpdate } })
end

local function chunk(session, text)
	update(session, { sessionUpdate = "agent_message_chunk", content = { type = "text", text = text } })
end

local OPTIONS = {
	{ optionId = "yes", name = "Allow", kind = "allow_once" },
	{ optionId = "no", name = "Reject", kind = "reject_once" },
}

local function turn(session, prompt)
	local path
	for _, block in ipairs(prompt) do
		if block.type == "resource" and block.resource.uri:match("^file://") then
			path = vim.uri_to_fname(block.resource.uri)
		end
	end

	chunk(session, "Rewriting")
	chunk(session, " the bo")
	chunk(session, "dy.\nDone soon.")

	update(session, { sessionUpdate = "tool_call", toolCallId = "run", title = "Run ls", kind = "execute" })
	call("session/request_permission", { sessionId = session, toolCall = { toolCallId = "run" }, options = OPTIONS })
	update(session, {
		sessionUpdate = "tool_call",
		toolCallId = "edit",
		title = "Edit the Draft",
		kind = "edit",
		locations = { { path = path } },
	})

	call("session/request_permission", { sessionId = session, toolCall = { toolCallId = "edit" }, options = OPTIONS })

	call("fs/write_text_file", { sessionId = session, path = "/tmp/elsewhere.txt", content = "x" })
	local read = call("fs/read_text_file", { sessionId = session, path = path })
	call("fs/write_text_file", { sessionId = session, path = path, content = read.result.content:upper() })
end

record({ pid = vim.uv.os_getpid() })
while true do
	local message = receive()
	if message.method == "initialize" then
		local login = { id = "stub-login", name = "Log in", description = "Run `acp-stub login`." }
		send({
			id = message.id,
			result = {
				protocolVersion = 1,
				agentCapabilities = { promptCapabilities = { embeddedContext = true } },
				authMethods = { login },
			},
		})
	elseif message.method == "session/new" and locked then
		send({ id = message.id, error = { code = -32000, message = "Authentication required" } })
	elseif message.method == "session/new" then
		send({ id = message.id, result = { sessionId = "stub" } })
	elseif message.method == "session/prompt" then
		turn(message.params.sessionId, message.params.prompt)
		send({ id = message.id, result = { stopReason = "end_turn" } })
	end
end
