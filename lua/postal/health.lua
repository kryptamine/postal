-- `:checkhealth postal`: whether `postal` and each agent can run, and
-- whether `postal` is the release the plugin came with.
local agent = require("postal.agent")
local recipients = require("postal.recipients")
local requirements = require("postal.requirements")

local M = {}

-- The plugin is the repository, so its Cargo.toml says its release.
local CARGO = vim.fs.joinpath(vim.fn.fnamemodify(debug.getinfo(1, "S").source:sub(2), ":p:h:h:h"), "Cargo.toml")

local function plugin_version()
	local ok, lines = pcall(vim.fn.readfile, CARGO)
	for _, line in ipairs(ok and lines or {}) do
		local version = line:match('^version%s*=%s*"([^"]+)"')
		if version then
			return version
		end
	end
end

local function versions()
	local plugin = plugin_version()
	local ok, result = pcall(function()
		return vim.system({ recipients.cmd, "--version" }, { text = true }):wait()
	end)

	local postal = ok and result.code == 0 and result.stdout:match("^postal (%S+)")
	if not plugin or not postal then
		vim.health.warn("cannot tell whether `" .. recipients.cmd .. "` is the plugin's release")
	elseif plugin == postal then
		vim.health.ok("`" .. recipients.cmd .. "` is " .. postal .. ", as the plugin")
	else
		vim.health.warn(
			("`%s` is %s and the plugin %s: install the plugin on the tag of the postal you run"):format(
				recipients.cmd,
				postal,
				plugin
			)
		)
	end
end

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

	versions()

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
