-- The MCP server an agent reaches a mail through (ADR 0020), run as
-- `nvim -l mcp.lua <servername> <buffer> <kind>`: JSON-RPC over stdio, one
-- message per line. The editor that owns the mail lists and runs its tools.
local server, mail, kind = arg[1], tonumber(arg[2]), arg[3]
local editor = vim.fn.sockconnect("pipe", server, { rpc = true })

local function send(message)
	message.jsonrpc = "2.0"
	io.stdout:write(vim.json.encode(message), "\n")
	io.stdout:flush()
end

-- Runs Lua `code` in the editor that owns the mail, on the arguments after it.
local function ask(code, ...)
	return vim.rpcrequest(editor, "nvim_exec_lua", code, { ... })
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
		result = { tools = ask("return require('postal.tools').list(...)", kind) }
	elseif message.method == "tools/call" then
		local params = message.params
		local code = "return require('postal.agent').serve(...)"
		result = ask(code, mail, params.name, params.arguments or vim.empty_dict())
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
