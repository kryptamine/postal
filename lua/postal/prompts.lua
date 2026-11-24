-- The instructions `:PostalAgent <name>` sends without typing, by name,
-- each for the kinds of mail it makes sense over; the user's for any.
local M = {}

local WRITTEN = { draft = true, signature = true }

local DEFAULTS = {
	proofread = {
		text = "Correct the spelling, grammar and punctuation, and change nothing else.",
		kinds = WRITTEN,
	},
	shorter = { text = "Make it shorter, keeping every point it makes.", kinds = WRITTEN },
	formal = { text = "Make it more formal.", kinds = WRITTEN },
	summary = { text = "Sum it up in a few lines.", kinds = { message = true } },
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

--- The text of each prompt over mail of `kind`, by name.
function M.over(kind)
	local texts = {}
	for name, prompt in pairs(by_name) do
		if not prompt.kinds or prompt.kinds[kind] then
			texts[name] = prompt.text
		end
	end

	return texts
end

return M
