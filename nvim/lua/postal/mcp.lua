-- The MCP server an agent edits a Draft through (ADR 0020), run as
-- `nvim -l mcp.lua <servername> <buffer>`: JSON-RPC over stdio, one message
-- per line, each tool call passed to the editor that owns the Draft.
local server, draft = arg[1], tonumber(arg[2])
local editor = vim.fn.sockconnect("pipe", server, { rpc = true })

local TOOLS = {
	{
		name = "read_draft",
		description = "The body of the email Draft as it stands, without its headers.",
		inputSchema = { type = "object", properties = vim.empty_dict() },
	},
	{
		name = "write_draft",
		description = "Replace the Draft's body; the user reviews the change hunk by hunk.",
		inputSchema = {
			type = "object",
			properties = { body = { type = "string", description = "The whole new body." } },
			required = { "body" },
		},
	},
}

local function send(message)
	message.jsonrpc = "2.0"
	io.stdout:write(vim.json.encode(message), "\n")
	io.stdout:flush()
end

local function call(params)
	local answer = vim.rpcrequest(
		editor,
		"nvim_exec_lua",
		"return require('postal.agent').serve(...)",
		{ draft, params.name, params.arguments or vim.empty_dict() }
	)

	return { content = { { type = "text", text = answer.text } }, isError = answer.failed }
end

for line in io.lines() do
	local message = vim.json.decode(line, { luanil = { object = true, array = true } })
	local result
	if message.method == "initialize" then
		result = {
			protocolVersion = message.params.protocolVersion,
			capabilities = { tools = vim.empty_dict() },
			serverInfo = { name = "postal", version = "0" },
		}
	elseif message.method == "tools/list" then
		result = { tools = TOOLS }
	elseif message.method == "tools/call" then
		result = call(message.params)
	elseif message.method == "ping" then
		result = vim.empty_dict()
	end

	-- A notification, `notifications/initialized` among them, takes no answer.
	if message.id ~= nil and result then
		send({ id = message.id, result = result })
	elseif message.id ~= nil then
		send({ id = message.id, error = { code = -32601, message = message.method .. " is not offered" } })
	end
end
