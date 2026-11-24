-- The instructions `:PostalAgent <name>` sends without typing, by name: a
-- default over mail the user `edits` or over mail only read, the user's own
-- over any.
local M = {}

local DEFAULTS = {
	proofread = {
		text = "Correct the spelling, grammar and punctuation, and change nothing else.",
		edits = true,
	},
	shorter = { text = "Make it shorter, keeping every point it makes.", edits = true },
	formal = { text = "Make it more formal.", edits = true },
	summary = { text = "Sum it up in a few lines.", edits = false },
}

local by_name = vim.deepcopy(DEFAULTS)

--- Starts from the defaults again, then takes `prompts`, `{ name = text }`;
--- a name set to false drops that default.
function M.set(prompts)
	by_name = vim.deepcopy(DEFAULTS)
	for name, text in pairs(prompts or {}) do
		if text == false then
			by_name[name] = nil
		elseif type(text) == "string" then
			by_name[name] = { text = text }
		else
			vim.notify("postal: the prompt " .. tostring(name) .. " is a string or false", vim.log.levels.ERROR)
		end
	end
end

--- The text of each prompt over mail the user `edits`, or does not, by name.
function M.over(edits)
	local texts = {}
	for name, prompt in pairs(by_name) do
		if prompt.edits == nil or prompt.edits == edits then
			texts[name] = prompt.text
		end
	end

	return texts
end

return M
