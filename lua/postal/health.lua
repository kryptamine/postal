-- `:checkhealth postal`: whether `postal` and each agent can run, whether
-- `postal` is the plugin's release, and what shows a Markdown Draft.
local config = require("postal.config")
local requirements = require("postal.requirements")
local version = require("postal.version")

local M = {}

local function versions()
	local ok, result = pcall(function()
		return vim.system({ config.options.cmd, "--version" }, { text = true }):wait()
	end)

	local postal = ok and result.code == 0 and result.stdout:match("^postal (%S+)")
	if not postal then
		vim.health.warn("cannot tell whether `" .. config.options.cmd .. "` is the plugin's release")
	elseif postal == version then
		vim.health.ok("`" .. config.options.cmd .. "` is " .. postal .. ", as the plugin")
	else
		vim.health.warn(
			("`%s` is %s and the plugin %s: install the plugin on the tag of the postal you run"):format(
				config.options.cmd,
				postal,
				version
			)
		)
	end
end

function M.check()
	vim.health.start("postal")
	local ok, result = pcall(function()
		return vim.system({ config.options.cmd, "thread", "show", "--help" }, { text = true }):wait()
	end)

	if ok and result.code == 0 then
		vim.health.ok("`" .. config.options.cmd .. "` runs `postal thread show`")
	else
		vim.health.error(requirements.thread(config.options.cmd, ok and result.stderr or tostring(result)))
	end

	versions()

	vim.health.start("postal markdown")
	if not vim.treesitter.language.add("markdown") then
		vim.health.warn("no treesitter parser for markdown: a Markdown Draft shows as plain text")
	elseif pcall(require, "render-markdown") then
		vim.health.ok("render-markdown.nvim renders a Markdown Draft's body")
	else
		vim.health.ok("a Markdown Draft's body is highlighted; render-markdown.nvim would render it")
	end

	vim.health.start("postal agents")
	for name, spec in vim.spairs(config.options.agents) do
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
