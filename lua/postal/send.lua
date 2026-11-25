-- `:PostalSend`: writes the Draft and exits the editor with the status
-- that has Postal send it, given in `$POSTAL_SEND_STATUS` (ADR 0037).
local agent = require("postal.agent")
local checks = require("postal.checks")

local M = {}

-- What holds a Send back, else nil; `bang` lets a warning through.
local function refusal(buf, bang)
	if agent.reviewing(buf) then
		return "the agent's edit waits for review"
	end

	checks.run(buf)
	local severity = bang and vim.diagnostic.severity.ERROR or { min = vim.diagnostic.severity.WARN }
	local found = vim.diagnostic.get(buf, { namespace = checks.NS, severity = severity })
	if #found > 0 then
		local hint = bang and "" or "; :PostalSend! sends anyway"
		return found[1].message .. hint
	end
end

--- Adds `:PostalSend` to Draft buffer `buf`, where Postal waits for it.
function M.command(buf)
	local status = tonumber(vim.env.POSTAL_SEND_STATUS)
	if not status then
		return
	end

	vim.api.nvim_buf_create_user_command(buf, "PostalSend", function(command)
		local why = refusal(buf, command.bang)
		if why then
			vim.notify("postal: not sent: " .. why, vim.log.levels.ERROR)
			return
		end

		vim.api.nvim_buf_call(buf, function()
			vim.cmd.write()
		end)
		vim.cmd("cquit " .. status)
	end, { bang = true, desc = "Send the Draft, as Postal's y does" })
end

return M
