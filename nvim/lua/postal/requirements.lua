-- What the plugin needs outside Neovim, and what to tell the user when it
-- is missing: shared by the chat and `:checkhealth postal`.
local M = {}

--- How agent `name`, `{ cmd = { ... }, npx = "..." }`, starts: `{ cmd = ...,
--- note = ... }`, the note set when it runs through npx; nil and why when
--- nothing can start it.
function M.agent(name, spec)
	local program = spec.cmd[1]
	if vim.fn.executable(program) == 1 then
		return { cmd = spec.cmd }
	end

	if spec.npx and vim.fn.executable("npx") == 1 then
		local note = ("`%s` is not on PATH, so npx fetches %s; the first start takes a while."):format(program, spec.npx)
		return { cmd = { "npx", "-y", spec.npx }, note = note }
	end

	if spec.npx then
		return nil,
			("%s needs `%s` or Node.js's `npx`, and neither is on PATH. Install Node.js, or run: npm install -g %s"):format(
				name,
				program,
				spec.npx
			)
	end

	return nil,
		("%s needs `%s`, which is not on PATH. Install it, or point `agents.%s.cmd` in setup() at it."):format(
			name,
			program,
			name
		)
end

--- What a failed `postal thread show` run by `cmd` means for the user.
function M.thread(cmd, stderr)
	if vim.fn.executable(cmd) == 0 then
		return ("`%s` is not on PATH: install Postal, or set `cmd` in setup()."):format(cmd)
	end

	if stderr:find("unrecognized subcommand", 1, true) then
		return ("`%s` predates `postal thread show`: install a newer Postal."):format(cmd)
	end

	return vim.trim(stderr)
end

return M
