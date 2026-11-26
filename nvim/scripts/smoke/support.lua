-- What every section of the smoke check shares: the plugin set up against
-- the stubs, where Drafts go, and how to read what the agent was sent.
local s = {}

function s.fail(msg)
	io.stderr:write("smoke: " .. msg .. "\n")
	vim.cmd("cquit 1")
end

local stub = vim.fn.fnamemodify("scripts/postal-stub", ":p")
s.acp_stub = vim.fn.fnamemodify("scripts/acp-stub.lua", ":p")
s.agent_log = vim.fn.tempname()
local function acp(arg)
	return { cmd = { vim.v.progpath, "--clean", "-l", s.acp_stub, arg } }
end

s.setup = {
	cmd = os.getenv("POSTAL_BIN") or stub,
	agents = {
		stub = acp(s.agent_log),
		locked = acp("locked"),
		absent = { cmd = { "postal-no-such-agent" } },
		fetched = { cmd = { "postal-no-such-agent" }, npx = "postal-acp-package" },
	},
	agent = "stub",
	keys = { stop = "<C-q>", close = false },
	spell = true,
	prompts = { shout = "Write it in capitals.", formal = false },
}

s.drafts = vim.fn.tempname() .. "/postal/drafts"
vim.fn.mkdir(s.drafts, "p")

--- The diagnostics of `code` over the current buffer, each as
--- `{ lnum, col, severity, message }`.
function s.diagnosed(code)
	local checks = vim.diagnostic.get(0, { namespace = vim.api.nvim_get_namespaces()["postal-checks"] })
	return vim.tbl_map(
		function(d)
			return { d.lnum, d.col, d.severity, d.message }
		end,
		vim.tbl_filter(function(d)
			return d.code == code
		end, checks)
	)
end

function s.said(buf)
	return table.concat(vim.api.nvim_buf_get_lines(buf, 0, -1, false), "\n")
end

-- Back to the window that shows a Draft, where `:PostalAgent` is.
function s.home()
	for _, win in ipairs(vim.api.nvim_list_wins()) do
		if vim.b[vim.api.nvim_win_get_buf(win)].postal_kind == "draft" then
			vim.api.nvim_set_current_win(win)
			return
		end
	end
end

--- Each message the stub agents logged, in the order they came.
function s.sent()
	local messages = {}
	for _, line in ipairs(vim.fn.filereadable(s.agent_log) == 1 and vim.fn.readfile(s.agent_log) or {}) do
		table.insert(messages, vim.json.decode(line, { luanil = { object = true, array = true } }))
	end

	return messages
end

function s.prompts()
	return vim.tbl_filter(function(message)
		return message.method == "session/prompt"
	end, s.sent())
end

function s.answers()
	return vim.tbl_filter(function(message)
		return message.id and not message.method
	end, s.sent())
end

return s
