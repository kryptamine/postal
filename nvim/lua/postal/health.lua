-- `:checkhealth postal`: whether `postal` and each agent can run.
local agent = require("postal.agent")
local recipients = require("postal.recipients")
local requirements = require("postal.requirements")

local M = {}

function M.check()
	vim.health.start("postal")
	local ok, result = pcall(function()
		return vim.system({ recipients.cmd, "thread", "show", "--help" }, { text = true }):wait()
	end)

	if ok and result.code == 0 then
		vim.health.ok("`" .. recipients.cmd .. "` runs `postal thread show`")
	else
		vim.health.error(requirements.thread(recipients.cmd, ok and result.stderr or tostring(result)))
	end

	vim.health.start("postal agents")
	for name, spec in vim.spairs(agent.agents) do
		local start, missing = requirements.agent(name, spec)
		if not start then
			vim.health.error(missing)
		elseif start.note then
			vim.health.warn(start.note)
		else
			vim.health.ok(name .. ": `" .. spec.cmd[1] .. "` found")
		end
	end
end

return M
