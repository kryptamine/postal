-- An ACP agent for `make smoke`, run by `nvim -l`: each turn streams a
-- message, asks to run a command, to read the Draft's file and to write
-- through the client's MCP server, which upper-cases the body it was sent,
-- a line `drop me` dropped. `locked` wants a sign-in. Over a Message, a
-- turn tries to write, then reads it and says its subject.
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

-- The client's MCP server, started with the session.
local mcp = { next_id = 0, answers = {}, pending = "" }

function mcp.start(server)
	local cmd = vim.list_extend({ server.command }, server.args)
	mcp.job = vim.system(cmd, {
		stdin = true,
		stderr = function(_, data)
			mcp.err = (mcp.err or "") .. (data or "")
		end,
		stdout = function(_, data)
			mcp.pending = mcp.pending .. (data or "")
			for line in mcp.pending:gmatch("([^\n]*)\n") do
				local message = vim.json.decode(line, { luanil = { object = true, array = true } })
				mcp.answers[message.id] = message
			end

			mcp.pending = mcp.pending:match("([^\n]*)$")
		end,
	})
end

function mcp.call(method, params)
	mcp.next_id = mcp.next_id + 1
	local id = mcp.next_id
	mcp.job:write(vim.json.encode({ jsonrpc = "2.0", id = id, method = method, params = params }) .. "\n")
	assert(
		vim.wait(5000, function()
			return mcp.answers[id] ~= nil
		end, 5),
		"the MCP server never answered " .. method .. ": " .. tostring(mcp.err)
	)
	return mcp.answers[id].result
end

local OPTIONS = {
	{ optionId = "yes", name = "Allow", kind = "allow_once" },
	{ optionId = "no", name = "Reject", kind = "reject_once" },
}

-- Asks to write and to read the Message, then says the subject it read.
local function read_message(session)
	local claude = { claudeCode = { toolName = "mcp__postal__write_draft" } }
	local write = { toolCallId = "write", title = "postal - write_draft (MCP)", _meta = claude }
	call("session/request_permission", { sessionId = session, toolCall = write, options = OPTIONS })
	local read = { toolCallId = "read", title = "mcp__postal__read_message", kind = "other" }
	call("session/request_permission", { sessionId = session, toolCall = read, options = OPTIONS })
	local written = mcp.call("tools/call", { name = "write_draft", arguments = { body = "gone" } })
	local text = mcp.call("tools/call", { name = "read_message", arguments = vim.empty_dict() }).content[1].text
	local subject = text:match("\nSubject: ([^\n]*)") or "none"
	chunk(session, "It is about " .. subject .. (written.isError and "." or ", and I changed it."))
end

-- Reads the Signature and writes it back in capitals.
local function sign(session)
	local claude = { claudeCode = { toolName = "mcp__postal__write_signature" } }
	local write = { toolCallId = "write", title = "postal - write_signature (MCP)", _meta = claude }
	call("session/request_permission", { sessionId = session, toolCall = write, options = OPTIONS })
	local read = mcp.call("tools/call", { name = "read_signature", arguments = vim.empty_dict() })
	local text = read.content[1].text:upper()
	mcp.call("tools/call", { name = "write_signature", arguments = { signature = text } })
	chunk(session, "Signed.")
end

local function turn(session, prompt)
	if mcp.message then
		read_message(session)
		return
	elseif mcp.signature then
		sign(session)
		return
	end

	local path, sent
	for _, block in ipairs(prompt) do
		if block.type == "resource" and block.resource.uri:match("^postal%-draft://") then
			path = block.resource.uri:gsub("^postal%-draft://", "")
			sent = block.resource.text
		end
	end

	-- Told `hang`, it says a word and waits for the turn to be cancelled.
	if prompt[#prompt].text == "hang" then
		chunk(session, "Thinking")
		repeat
			local message = receive()
		until message.method == "session/cancel"

		chunk(session, " no more.")
		return "cancelled"
	end

	chunk(session, "Rewriting")
	chunk(session, " the bo")
	chunk(session, "dy.\nDone soon.")

	update(session, { sessionUpdate = "tool_call", toolCallId = "run", title = "Run ls", kind = "execute" })
	call("session/request_permission", { sessionId = session, toolCall = { toolCallId = "run" }, options = OPTIONS })
	update(session, {
		sessionUpdate = "tool_call",
		toolCallId = "read",
		title = "Read " .. path,
		kind = "read",
		locations = { { path = path } },
	})

	call("session/request_permission", { sessionId = session, toolCall = { toolCallId = "read" }, options = OPTIONS })
	update(session, { sessionUpdate = "tool_call", toolCallId = "write", title = "write_draft", kind = "other" })
	local claude = { claudeCode = { toolName = "mcp__postal__write_draft" } }
	local write = { toolCallId = "write", title = "postal - write_draft (MCP)", _meta = claude }
	call("session/request_permission", { sessionId = session, toolCall = write, options = OPTIONS })
	-- An older claude-agent-acp names the tool in the title alone.
	local older = { toolCallId = "read_draft", title = "mcp__postal__read_draft", kind = "other" }
	call("session/request_permission", { sessionId = session, toolCall = older, options = OPTIONS })
	local read = mcp.call("tools/call", { name = "read_draft", arguments = vim.empty_dict() })
	-- Told `slowly`, it writes once the smoke check has edited the Draft.
	if prompt[#prompt].text == "slowly" then
		while not vim.uv.fs_stat(log .. ".go") do
			vim.uv.sleep(10)
		end
	end

	local lines = vim.tbl_filter(function(line)
		return line ~= "drop me"
	end, vim.split(sent, "\n", { plain = true }))

	-- A read that gives other than the body sent shows in the Draft; told
	-- `slowly`, the user's edit may have come first.
	local text = table.concat(lines, "\n"):upper()
	if read.content[1].text ~= sent and prompt[#prompt].text ~= "slowly" then
		text = "read " .. read.content[1].text
	end

	mcp.call("tools/call", { name = "write_draft", arguments = { body = text } })
	-- Told `quit`, it ends as soon as it has written.
	if prompt[#prompt].text == "quit" then
		os.exit(0)
	end
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
		mcp.start(message.params.mcpServers[1])
		mcp.call("initialize", { protocolVersion = "2025-06-18", capabilities = vim.empty_dict() })
		local tools = vim.tbl_map(function(tool)
			return tool.name
		end, mcp.call("tools/list", vim.empty_dict()).tools)

		mcp.message = vim.deep_equal(tools, { "read_message" })
		mcp.signature = vim.deep_equal(tools, { "read_signature", "write_signature" })
		local draft = vim.deep_equal(tools, { "read_draft", "write_draft" })
		assert(mcp.message or mcp.signature or draft, vim.inspect(tools))
		send({ id = message.id, result = { sessionId = "stub" } })
	elseif message.method == "session/prompt" then
		local stopped = turn(message.params.sessionId, message.params.prompt)
		send({ id = message.id, result = { stopReason = stopped or "end_turn" } })
	end
end
