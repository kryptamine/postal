-- A connection to an ACP agent: JSON-RPC 2.0 over its stdio, one message
-- per line (ADR 0018).
local M = {}

--- JSON-RPC's code for a method the client does not offer.
M.NOT_FOUND = -32601

--- ACP's code for an agent that needs its user to sign in first.
M.AUTH_REQUIRED = -32000

local Connection = {}
Connection.__index = Connection

local function decode(line)
	local ok, message = pcall(vim.json.decode, line, { luanil = { object = true, array = true } })
	return ok and type(message) == "table" and message or nil
end

--- Starts `cmd` in `cwd`; `handlers` answer `request(method, params,
--- respond)`, take `notify(method, params)` and `exit(code, stderr)`, on the
--- main loop. Returns nil and the error when `cmd` cannot start.
function M.start(cmd, cwd, handlers)
	local self = setmetatable({ next_id = 0, waiting = {}, handlers = handlers }, Connection)
	local pending, stderr = "", ""
	local ok, job = pcall(vim.system, cmd, {
		cwd = cwd,
		stdin = true,
		stdout = function(_, data)
			pending = pending .. (data or "")
			while true do
				local line, rest = pending:match("^([^\n]*)\n(.*)$")
				if not line then
					break
				end

				pending = rest
				vim.schedule(function()
					self:receive(line)
				end)
			end
		end,
		stderr = function(_, data)
			-- The tail is enough to say why an agent ended.
			stderr = (stderr .. (data or "")):sub(-2000)
		end,
	}, function(result)
		vim.schedule(function()
			self.job = nil
			handlers.exit(result.code, stderr)
		end)
	end)

	if not ok then
		return nil, tostring(job)
	end

	self.job = job
	return self
end

function Connection:write(message)
	if self.job then
		message.jsonrpc = "2.0"
		-- An agent gone mid-write is told by `handlers.exit`.
		pcall(self.job.write, self.job, vim.json.encode(message) .. "\n")
	end
end

--- Sends `method` and calls `callback(error, result)` with its answer,
--- `error` holding the agent's `code` and `message`.
function Connection:request(method, params, callback)
	self.next_id = self.next_id + 1
	self.waiting[self.next_id] = callback
	self:write({ id = self.next_id, method = method, params = params })
end

function Connection:receive(line)
	local message = decode(line)
	if not message then
		return
	end

	if message.method and message.id ~= nil then
		self.handlers.request(message.method, message.params or {}, function(result, error)
			if error then
				self:write({ id = message.id, error = error })
			else
				self:write({ id = message.id, result = result == nil and vim.NIL or result })
			end
		end)
	elseif message.method then
		self.handlers.notify(message.method, message.params or {})
	elseif message.id ~= nil then
		local callback = self.waiting[message.id]
		self.waiting[message.id] = nil
		if callback then
			callback(message.error, message.result)
		end
	end
end

--- Ends the agent; `handlers.exit` still runs.
function Connection:close()
	local job = self.job
	self.job = nil
	if job then
		-- An agent blocked reading its stdin may never see the signal.
		pcall(job.write, job, nil)
		job:kill(15)
	end
end

return M
