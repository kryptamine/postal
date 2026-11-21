-- Run by `make smoke` and tests/nvim.rs: in a bare nvim, a Draft's `Cc:`
-- completes a recipient from Postal's Correspondents, and its body nothing;
-- `:PostalAgent` chats with a stub ACP agent about a Draft and a Message.
local function fail(msg)
	io.stderr:write("smoke: " .. msg .. "\n")
	vim.cmd("cquit 1")
end

local stub = vim.fn.fnamemodify("scripts/postal-stub", ":p")
local acp_stub = vim.fn.fnamemodify("scripts/acp-stub.lua", ":p")
local agent_log = vim.fn.tempname()
local function acp(arg)
	return { cmd = { vim.v.progpath, "--clean", "-l", acp_stub, arg } }
end

require("postal").setup({
	cmd = os.getenv("POSTAL_BIN") or stub,
	agents = {
		stub = acp(agent_log),
		locked = acp("locked"),
		absent = { cmd = { "postal-no-such-agent" } },
		fetched = { cmd = { "postal-no-such-agent" }, npx = "postal-acp-package" },
	},
	agent = "stub",
})

local drafts = vim.fn.tempname() .. "/postal/drafts"
vim.fn.mkdir(drafts, "p")
local draft = drafts .. "/1.eml"
vim.fn.writefile({
	"To: bob@example.com",
	"Cc: Ada Lovelace <ada@example.com>, pet",
	"Subject: Hi",
	"",
	"pet",
}, draft)
vim.cmd.edit(draft)

if vim.bo.omnifunc ~= "v:lua.require'postal'.omnifunc" then
	fail("omnifunc not set, it is " .. vim.inspect(vim.bo.omnifunc))
end

-- Without blink, Neovim's own menu opens as a recipient is typed
if vim.fn.exists("+autocomplete") == 1 and not (vim.bo.autocomplete and vim.bo.complete == "o") then
	fail("autocomplete not fed by the omnifunc alone")
end

local omnifunc = require("postal").omnifunc
local function at(row, col)
	vim.api.nvim_win_set_cursor(0, { row, col })
	local start = omnifunc(1, "")
	if start < 0 then
		return start, {}
	end
	local base = vim.api.nvim_get_current_line():sub(start + 1, col)
	return start, omnifunc(0, base).words
end

-- The recipient after the comma, not the whole header
local before = "Cc: Ada Lovelace <ada@example.com>, "
local start, items = at(2, #before + #"pet")
if start ~= #before then
	fail("recipient starts at " .. start .. ", not " .. #before)
end

local words = vim.tbl_map(function(item)
	return item.word
end, items)
local want = { "Petr Novak <petr@example.com>", "Peter Pan <pan@example.com>" }
if not vim.deep_equal(words, want) then
	fail("offered " .. vim.inspect(words))
end

if at(5, #"pet") ~= -3 then
	fail("the body offered recipients")
end

local reply = drafts .. "/2.eml"
vim.fn.writefile({ "To: bob@example.com", "Subject: Re: Hi", "", "hi bob", "", "> Shall we meet?" }, reply)
vim.cmd.edit(reply)
local draft_buf = vim.api.nvim_get_current_buf()
local function body()
	return vim.api.nvim_buf_get_lines(draft_buf, 2, -1, false)
end

-- Back to the window that shows a Draft, where `:PostalAgent` is.
local function home()
	for _, win in ipairs(vim.api.nvim_list_wins()) do
		if vim.b[vim.api.nvim_win_get_buf(win)].postal_kind == "draft" then
			vim.api.nvim_set_current_win(win)
			return
		end
	end
end

-- Types `text` in the pane's input and sends it, as the user would.
local function ask(text)
	home()
	vim.cmd("PostalAgent")
	vim.api.nvim_buf_set_lines(0, -2, -1, false, { text })
	vim.cmd.normal(vim.keycode("<CR>"))
end

local function sent()
	local messages = {}
	for _, line in ipairs(vim.fn.filereadable(agent_log) == 1 and vim.fn.readfile(agent_log) or {}) do
		table.insert(messages, vim.json.decode(line, { luanil = { object = true, array = true } }))
	end

	return messages
end

local function prompts()
	return vim.tbl_filter(function(message)
		return message.method == "session/prompt"
	end, sent())
end

local function answers()
	return vim.tbl_filter(function(message)
		return message.id and not message.method
	end, sent())
end

-- Each turn the stub asks four times
local function turn(want)
	if not vim.wait(10000, function()
		return vim.deep_equal(body(), want) and #answers() == 4 * #prompts()
	end) then
		fail("the body is " .. vim.inspect(body()) .. ", not " .. vim.inspect(want))
	end
end

ask("make it formal")
turn({ "", "HI BOB", "", "> SHALL WE MEET?" })
if vim.api.nvim_buf_get_lines(draft_buf, 0, 2, false)[1] ~= "To: bob@example.com" then
	fail("the headers changed")
end

local function said(buf)
	return table.concat(vim.api.nvim_buf_get_lines(buf, 0, -1, false), "\n")
end

local function right_pane()
	return vim.fn.winbufnr(vim.fn.winnr("l"))
end

local pane_buf = right_pane()
local pane = table.concat(vim.api.nvim_buf_get_lines(pane_buf, 0, -1, false), "\n")
local wants = {
	"## Me\n\nmake it formal\n",
	"## stub\n\nRewriting the body.\nDone soon.\n",
	"✗ Denied: Run ls\n✗ Denied: Read …/drafts/2.eml\n",
	"✓ write_draft · the edit is in the Draft for review",
}

for _, said in ipairs(wants) do
	if not pane:find(said, 1, true) then
		fail("the pane lacks " .. vim.inspect(said) .. " in " .. vim.inspect(pane))
	end
end

-- The pane's `winbar` or `statusline` as it is drawn.
local function bar(option)
	local win = vim.fn.bufwinid(pane_buf)
	return vim.api.nvim_eval_statusline(vim.wo[win][option], { winid = win, use_winbar = option == "winbar" }).str
end

-- The pane names its agent and the mail it is about
local status = bar("statusline")
if not status:find("stub · Re: Hi", 1, true) then
	fail("the pane's statusline is " .. vim.inspect(status))
end

local first = prompts()[1].params.prompt
local thread = vim.tbl_filter(function(block)
	return block.type == "resource" and block.resource.uri:match("^postal%-thread:")
end, first)

local types = vim.tbl_map(function(block)
	return block.type
end, first)

if not first[1].text:find("write_draft", 1, true) or not first[1].text:find("Never send") then
	fail("the first prompt lacks the Draft's rules: " .. vim.inspect(first[1]))
end

-- With the real binary the Draft answers no Message, so there is no Thread,
-- and nothing in the pane says so
if os.getenv("POSTAL_BIN") and pane:find("No Thread", 1, true) then
	fail("a new mail's empty Thread was reported: " .. vim.inspect(pane))
end

local want_types = os.getenv("POSTAL_BIN") and { "text", "resource", "text" }
	or { "text", "resource", "resource", "text" }

if not vim.deep_equal(types, want_types) then
	fail("the first prompt holds " .. vim.inspect(types))
end

if not os.getenv("POSTAL_BIN") and not thread[1].resource.text:find("Shall we meet?", 1, true) then
	fail("the Thread is " .. vim.inspect(thread))
end

if first[#first - 1].resource.text ~= "hi bob\n\n> Shall we meet?\n" or first[#first].text ~= "make it formal" then
	fail("the first prompt's body and instruction: " .. vim.inspect(first))
end

local outcomes = vim.tbl_map(function(message)
	return message.result.outcome.optionId
end, answers())

-- a command and the Draft's file denied, Postal's MCP tools allowed
if not vim.deep_equal(outcomes, { "no", "no", "yes", "yes" }) then
	fail("the agent was answered " .. vim.inspect(outcomes))
end

local review = vim.api.nvim_create_namespace("postal-review")
-- What the review shows over the Draft as virtual lines, top to bottom.
local function virtual()
	local lines = {}
	for _, mark in ipairs(vim.api.nvim_buf_get_extmarks(draft_buf, review, 0, -1, { details = true })) do
		for _, line in ipairs(mark[4].virt_lines or {}) do
			table.insert(lines, line[1][1])
		end
	end

	return lines
end

local function shows(want)
	if not vim.deep_equal(virtual(), want) then
		fail("the review shows " .. vim.inspect(virtual()) .. ", not " .. vim.inspect(want))
	end
end

local function left(want)
	local winbar = bar("winbar")
	if want == "" and winbar:find("hunk") or not winbar:find(want, 1, true) then
		fail("the pane says " .. vim.inspect(winbar) .. ", not " .. vim.inspect(want))
	end
end

local function press(row, lhs)
	home()
	vim.api.nvim_win_set_cursor(0, { row, 0 })
	vim.cmd.normal(vim.keycode(lhs))
end

local function is(want)
	if not vim.deep_equal(body(), want) then
		fail("the body is " .. vim.inspect(body()) .. ", not " .. vim.inspect(want))
	end
end

-- The agent's lines are in the buffer, the ones they replace shown over it
shows({ "hi bob", "> Shall we meet?" })
left("2 hunks left")
left("? keys")
-- A narrow pane drops whole keys, never the count or the key list
vim.api.nvim_win_set_width(vim.fn.bufwinid(pane_buf), 46)
left("2 hunks left  \\a accept ")
if bar("winbar"):find("reject", 1, true) or not vim.endswith(bar("winbar"), "? keys ") then
	fail("the narrow winbar is " .. vim.inspect(bar("winbar")))
end

vim.cmd.wincmd("=")
press(4, "<leader>a")
left("1 hunk left")
press(6, "<leader>r")
is({ "", "HI BOB", "", "> Shall we meet?" })
shows({})
left("")

-- A hand edit goes with the next turn, which carries no rules again
vim.api.nvim_buf_set_lines(draft_buf, 3, 4, false, { "hello bob" })
vim.api.nvim_buf_set_lines(draft_buf, -1, -1, false, { "", "drop me" })
ask("shorter")
turn({ "", "HELLO BOB", "", "> SHALL WE MEET?", "" })
shows({ "hello bob", "> Shall we meet?", "drop me" })
local second = prompts()[2].params.prompt
local asked = "hello bob\n\n> Shall we meet?\n\ndrop me\n"
if #second ~= 2 or second[1].resource.text ~= asked or second[2].text ~= "shorter" then
	fail("the second prompt is " .. vim.inspect(second))
end

-- An instruction waits for the review of the agent's last edit
ask("slowly")
vim.wait(200)
if #prompts() ~= 2 then
	fail("prompted while a review was open")
end

press(4, "<leader>R")
is({ "", "hello bob", "", "> Shall we meet?", "", "drop me" })
shows({})
if not vim.wait(5000, function()
	return #prompts() == 3
end) then
	fail("the waiting instruction was never sent")
end

-- The user's edit while the agent works is never overwritten unasked
vim.api.nvim_buf_set_lines(draft_buf, 3, 4, false, { "hey bob" })
vim.fn.writefile({}, agent_log .. ".go")
turn({ "", "hey bob", "", "> SHALL WE MEET?", "" })
shows({ "HELLO BOB", "> Shall we meet?", "drop me" })
left("3 hunks left (1 conflict)")
press(4, "<leader>a")
left("2 hunks left")
press(4, "<leader>A")
is({ "", "HELLO BOB", "", "> SHALL WE MEET?", "" })
shows({})
left("")

-- An undo past the agent's edit ends its review, keeping the user's typing
-- before it, so no later decision writes over the wrong rows
vim.api.nvim_buf_set_lines(draft_buf, 3, 4, false, { "undo bob" })
ask("again")
turn({ "", "UNDO BOB", "", "> SHALL WE MEET?", "" })
left("1 hunk left")
home()
vim.cmd.undo()
if not vim.wait(5000, function()
	return said(pane_buf):find("The agent's edit was undone", 1, true) ~= nil
end) then
	fail("the undo left the review open: " .. vim.inspect(said(pane_buf)))
end

is({ "", "undo bob", "", "> SHALL WE MEET?", "" })
shows({})
left("")
if vim.fn.maparg("<leader>a", "n", false, true).buffer == 1 then
	fail("the review's keys outlived the undo")
end

-- `q` in the pane stops a turn, and the pane says so; a chunk after the
-- user deleted the blank line over the input still ends the agent's text
ask("hang")
if not vim.wait(5000, function()
	return said(pane_buf):find("Thinking", 1, true) ~= nil
end) then
	fail("hang was never answered")
end

local input = #vim.api.nvim_buf_get_lines(pane_buf, 0, -1, false) - 2
vim.api.nvim_buf_set_lines(pane_buf, input - 1, input, false, {})
vim.cmd.normal("q")
if not vim.wait(5000, function()
	return said(pane_buf):find("Thinking no more.\n> Stopped.", 1, true) ~= nil
end) then
	fail("the turn was not stopped: " .. vim.inspect(said(pane_buf)))
end

local tail = vim.api.nvim_buf_get_lines(pane_buf, -4, -1, false)
if not vim.deep_equal(tail, { "", "## Me", "" }) or not said(pane_buf):find("## Me\n\nhang\n", 1, true) then
	fail("the input is not left empty at the bottom: " .. vim.inspect(said(pane_buf)))
end

-- An agent wanting a sign-in shows its command
local locked = drafts .. "/3.eml"
vim.fn.writefile({ "To: bob@example.com", "", "hi" }, locked)
home()
vim.cmd.edit(locked)
local locked_buf = vim.api.nvim_get_current_buf()
vim.cmd("PostalAgent locked")
local locked_pane = vim.api.nvim_get_current_buf()
if
	not vim.wait(10000, function()
		local said = table.concat(vim.api.nvim_buf_get_lines(locked_pane, 0, -1, false), "\n")
		return said:find("Run `acp-stub login`.", 1, true) ~= nil
	end)
then
	fail("no sign-in shown: " .. vim.inspect(vim.api.nvim_buf_get_lines(locked_pane, 0, -1, false)))
end

vim.cmd("bwipeout! " .. locked_buf)

-- An agent off PATH says what to do, in the pane and in an error
local notified = {}
vim.notify = function(msg)
	table.insert(notified, msg)
end

local function chat(name)
	local file = drafts .. "/" .. name .. ".eml"
	vim.fn.writefile({ "To: bob@example.com", "", "hi" }, file)
	home()
	vim.cmd.edit(file)
	local buf = vim.api.nvim_get_current_buf()
	vim.cmd("PostalAgent " .. name)
	return buf, vim.api.nvim_get_current_buf()
end

local absent_buf, absent_pane = chat("absent")
local hint = "absent needs `postal-no-such-agent`, which is not on PATH"
if not said(absent_pane):find(hint, 1, true) or not (notified[1] or ""):find(hint, 1, true) then
	fail("no hint for a missing agent: " .. vim.inspect(said(absent_pane)) .. " " .. vim.inspect(notified))
end

vim.cmd("bwipeout! " .. absent_buf)

-- Off PATH, an agent with an npx package runs through npx
local bin = vim.fn.tempname()
vim.fn.mkdir(bin, "p")
vim.fn.writefile({
	"#!/bin/sh",
	'[ "$1 $2" = "-y postal-acp-package" ] || exit 3',
	("exec '%s' --clean -l '%s' locked"):format(vim.v.progpath, acp_stub),
}, bin .. "/npx")
vim.fn.setfperm(bin .. "/npx", "rwxr-xr-x")
vim.env.PATH = bin .. ":" .. vim.env.PATH
local fetched_buf, fetched_pane = chat("fetched")
if
	not vim.wait(10000, function()
		return said(fetched_pane):find("Run `acp-stub login`.", 1, true) ~= nil
	end) or not said(fetched_pane):find("npx fetches postal-acp-package", 1, true)
then
	fail("npx did not stand in: " .. vim.inspect(said(fetched_pane)))
end

vim.cmd("bwipeout! " .. fetched_buf)

-- A Message opened to read cannot be changed, by the user or the agent,
-- and the agent reads it whole
local messages = vim.fn.tempname() .. "/postal/messages"
vim.fn.mkdir(messages, "p")
local message_text = { "From: Ada <ada@example.com>", "Subject: Weekly notes", "", "See you soon." }
vim.fn.writefile(message_text, messages .. "/7.eml")
home()
vim.cmd.edit(messages .. "/7.eml")
local message_buf = vim.api.nvim_get_current_buf()
if vim.bo.modifiable or not vim.bo.readonly or not vim.wo.wrap then
	fail("the Message can be changed, or is not wrapped")
end

local before_message = #prompts()
vim.cmd("PostalAgent")
local message_pane = vim.api.nvim_get_current_buf()
vim.api.nvim_buf_set_lines(0, -2, -1, false, { "what is it about?" })
vim.cmd.normal(vim.keycode("<CR>"))
if not vim.wait(10000, function()
	return said(message_pane):find("It is about Weekly notes.", 1, true) ~= nil
end) then
	fail("the agent did not read the Message: " .. vim.inspect(said(message_pane)))
end

local asked = prompts()[before_message + 1].params.prompt
local message_blocks = vim.tbl_map(function(block)
	return block.resource and block.resource.uri:match("^[%w-]+") or block.text
end, asked)

local want_blocks = { asked[1].text, "postal-message", "what is it about?" }
if not vim.deep_equal(message_blocks, want_blocks) or not asked[1].text:find("read_message", 1, true) then
	fail("the Message's prompt holds " .. vim.inspect(asked))
end

if asked[2].resource.text ~= table.concat(message_text, "\n") .. "\n" then
	fail("the Message resource is " .. vim.inspect(asked[2].resource.text))
end

local message_outcomes = vim.tbl_map(function(message)
	return message.result.outcome.optionId
end, vim.list_slice(answers(), #answers() - 1))

if not vim.deep_equal(message_outcomes, { "no", "yes" }) then
	fail("over a Message the agent was answered " .. vim.inspect(message_outcomes))
end

vim.cmd("bwipeout! " .. message_buf)
if vim.api.nvim_buf_is_valid(message_pane) then
	fail("the pane outlived its Message")
end
vim.cmd("checkhealth postal")
local health = said(0)
for _, want in ipairs({ "runs `postal thread show`", hint, "npx fetches postal-acp-package" }) do
	if not health:find(want, 1, true) then
		fail("checkhealth lacks " .. vim.inspect(want) .. " in " .. vim.inspect(health))
	end
end

vim.cmd("bwipeout!")

-- The session ends with its Draft's buffer
vim.cmd("bwipeout! " .. draft_buf)
if vim.api.nvim_buf_is_valid(pane_buf) then
	fail("the pane outlived its Draft")
end

local pid = sent()[1].pid
if not vim.wait(5000, function()
	return vim.uv.kill(pid, 0) ~= 0
end) then
	fail("the agent outlived its Draft")
end

io.stdout:write("smoke: ok\n")
vim.cmd("qall!")
