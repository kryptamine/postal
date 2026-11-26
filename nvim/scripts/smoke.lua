-- Run by `make smoke` and tests/nvim.rs: the plugin in a bare nvim against
-- a stub `postal` and stub ACP agents. Each section builds on the state the
-- ones before it leave, so they run in this order.
local s = dofile("scripts/smoke/support.lua")
require("postal").setup(s.setup)

for _, section in ipairs({ "draft", "review", "start", "message", "signature", "markdown", "health", "ends" }) do
	assert(loadfile("scripts/smoke/" .. section .. ".lua"))(s)
end

io.stdout:write("smoke: ok\n")
vim.cmd("qall!")
