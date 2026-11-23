-- The demo's ACP agent, run by `nvim -l`: whatever it is asked, it streams
-- a short answer and rewrites the Draft's own text into a tidy reply to
-- Maya, the Quote below left as it was, so every take reviews the same hunks.
local REPLY = {
	"Hi Maya,",
	"",
	"Thanks for sorting out the plan. My flight lands in Lisbon at 14:30 on",
	"the 9th, so I'll take the 16:09 train and should reach Porto by 19:00.",
	"Could you save me a seat at dinner?",
	"",
	"See you there,",
	"Sam",
}

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

	return vim.json.decode(line, { luanil = { object = true, array = true } })
end

local next_id = 0
local function call(method, params)
	next_id = next_id + 1
	send({ id = next_id, method = method, params = params })
	repeat
		local message = receive()
	until message.id == next_id and not message.method
end

local function chunk(session, text)
	local content = { type = "text", text = text }
	send({
		method = "session/update",
		params = { sessionId = session, update = { sessionUpdate = "agent_message_chunk", content = content } },
	})
	vim.uv.sleep(60)
end

-- The client's MCP server, started with the session.
local mcp = { next_id = 0, answers = {}, pending = "" }

function mcp.start(server)
	mcp.job = vim.system(vim.list_extend({ server.command }, server.args), {
		stdin = true,
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
	vim.wait(5000, function()
		return mcp.answers[id] ~= nil
	end, 5)
	return mcp.answers[id].result
end

-- The Draft's text is what comes before the attribution line of its Quote.
local function rewrite(body)
	local lines = vim.split(body, "\n", { plain = true })
	local quote = #lines + 1
	for i, line in ipairs(lines) do
		if line:match("^On .* wrote:$") or line:match("^>") then
			quote = i
			break
		end
	end

	local out = vim.list_extend({}, REPLY)
	if quote <= #lines then
		table.insert(out, "")
		vim.list_extend(out, lines, quote)
	end

	return table.concat(out, "\n")
end

local function turn(session)
	for _, text in ipairs({ "Sure", ", I'll", " tidy it up", " and keep", " your times." }) do
		chunk(session, text)
	end

	local write = {
		toolCallId = "write",
		title = "postal - write_draft (MCP)",
		_meta = { claudeCode = { toolName = "mcp__postal__write_draft" } },
	}
	local options = {
		{ optionId = "yes", name = "Allow", kind = "allow_once" },
		{ optionId = "no", name = "Reject", kind = "reject_once" },
	}
	call("session/request_permission", { sessionId = session, toolCall = write, options = options })
	local read = mcp.call("tools/call", { name = "read_draft", arguments = vim.empty_dict() })
	mcp.call("tools/call", { name = "write_draft", arguments = { body = rewrite(read.content[1].text) } })
	chunk(session, "\nDone: review the hunks in the Draft.")
end

while true do
	local message = receive()
	if message.method == "initialize" then
		send({
			id = message.id,
			result = { protocolVersion = 1, agentCapabilities = { promptCapabilities = { embeddedContext = true } } },
		})
	elseif message.method == "session/new" then
		mcp.start(message.params.mcpServers[1])
		mcp.call("initialize", { protocolVersion = "2025-06-18", capabilities = vim.empty_dict() })
		send({ id = message.id, result = { sessionId = "demo" } })
	elseif message.method == "session/prompt" then
		turn(message.params.sessionId)
		send({ id = message.id, result = { stopReason = "end_turn" } })
	end
end
