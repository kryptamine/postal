-- The tools Postal's MCP server offers an agent over one Draft (ADR 0020):
-- what MCP lists, which permission requests they answer to, and their runs.
local M = {}

local TOOLS = {
	read_draft = {
		description = "The body of the email Draft as it stands, without its headers.",
		inputSchema = { type = "object" },
		run = function(chat)
			return chat:body()
		end,
	},
	write_draft = {
		description = "Replace the Draft's body; the user reviews the change hunk by hunk.",
		inputSchema = {
			type = "object",
			properties = { body = { type = "string", description = "The whole new body." } },
			required = { "body" },
		},
		run = function(chat, args)
			if type(args.body) ~= "string" then
				return nil, "`body` must be a string"
			end

			chat:propose(args.body)
			return "The new body is in the Draft for the user's review."
		end,
	},
}

--- Each tool as MCP's `tools/list` gives it, by name.
function M.list()
	local list = {}
	for name, tool in vim.spairs(TOOLS) do
		table.insert(list, { name = name, description = tool.description, inputSchema = tool.inputSchema })
	end

	return list
end

--- Whether permission request `call` is for one of these tools, known by its
--- exact name: Claude's `_meta`, else the title an older adapter sets to it.
function M.asked(call)
	local meta = call._meta and call._meta.claudeCode
	local name = meta and meta.toolName or call.title or ""
	return TOOLS[(name:gsub("^mcp__postal__", ""))] ~= nil
end

--- Runs tool `name` for `chat`, as MCP's `tools/call` answers it.
function M.run(chat, name, args)
	local tool = TOOLS[name]
	local text, failure
	if tool then
		text, failure = tool.run(chat, args)
	else
		failure = "no tool " .. tostring(name)
	end

	return { content = { { type = "text", text = text or failure } }, isError = text == nil }
end

return M
