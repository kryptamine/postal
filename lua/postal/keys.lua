-- The plugin's keys by name, each its default `lhs`; `setup({ keys = ... })`
-- maps one to another, or leaves it unmapped with false.
local M = {}

local DEFAULTS = {
	send = "<CR>",
	send_insert = "<C-s>",
	stop = "q",
	close = "<C-c>",
	help = "?",
	accept = "<leader>a",
	reject = "<leader>r",
	accept_all = "<leader>A",
	reject_all = "<leader>R",
}

M.lhs = vim.deepcopy(DEFAULTS)

--- Starts from the defaults again, then takes `keys`, `{ name = lhs }`;
--- a name it has not, or an `lhs` neither a string nor false, is refused.
function M.set(keys)
	M.lhs = vim.deepcopy(DEFAULTS)
	for name, lhs in pairs(keys or {}) do
		if DEFAULTS[name] == nil then
			vim.notify("postal: no key named " .. tostring(name) .. " in setup()", vim.log.levels.ERROR)
		elseif lhs ~= false and type(lhs) ~= "string" then
			vim.notify("postal: the key " .. name .. " is a string or false", vim.log.levels.ERROR)
		else
			M.lhs[name] = lhs
		end
	end
end

--- Each of `keys` whose `name` is mapped, with its `lhs`, in order and by
--- name; the others left out.
function M.bound(keys)
	local bound = {}
	for _, key in ipairs(keys) do
		local lhs = M.lhs[key.name]
		if lhs then
			local key_bound = vim.tbl_extend("force", key, { lhs = lhs })
			table.insert(bound, key_bound)
			bound[key.name] = key_bound
		end
	end

	return bound
end

--- Maps `bound` keys in buffer `buf`, in their `modes` or normal mode.
function M.map(buf, bound)
	for _, key in ipairs(bound) do
		vim.keymap.set(key.modes or "n", key.lhs, key.run, { buffer = buf, desc = key.desc })
	end
end

function M.unmap(buf, bound)
	for _, key in ipairs(bound) do
		pcall(vim.keymap.del, key.modes or "n", key.lhs, { buffer = buf })
	end
end

return M
