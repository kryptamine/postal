-- The options `setup()` takes, each its default until a call sets it; the
-- keys and the prompts check and merge their own in `postal.keys` and
-- `postal.prompts`.
local M = {}

local function defaults()
	return {
		cmd = "postal",
		agents = {
			claude = { cmd = { "claude-agent-acp" }, npx = "@agentclientprotocol/claude-agent-acp" },
		},
		agent = "claude",
		nerd_font = vim.g.have_nerd_font == true,
		spell = false,
		attachment_words = { "attach", "enclos" },
	}
end

M.options = defaults()

--- Starts from the defaults again, then takes `opts`: each option whole,
--- but `agents`, which adds to the defaults by name.
function M.set(opts)
	M.options = defaults()
	for name, value in pairs(opts or {}) do
		if name == "agents" then
			M.options.agents = vim.tbl_deep_extend("force", M.options.agents, value)
		elseif M.options[name] ~= nil then
			M.options[name] = value
		end
	end
end

return M
