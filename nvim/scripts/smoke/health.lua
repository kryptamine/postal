-- `:checkhealth postal` says what is missing and which release runs.
local s = ...
local fail = s.fail
local said = s.said
local hint = s.hint

vim.cmd("checkhealth postal")
local health = said(0)
-- A postal of another release than the plugin's is told
local release = os.getenv("POSTAL_BIN") and ", as the plugin" or "is 0.0.1 and the plugin "
for _, want in ipairs({ "runs `postal thread show`", hint, "npx fetches postal-acp-package", release }) do
	if not health:find(want, 1, true) then
		fail("checkhealth lacks " .. vim.inspect(want) .. " in " .. vim.inspect(health))
	end
end

vim.cmd("bwipeout!")
