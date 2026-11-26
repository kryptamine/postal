-- An agent edits a Draft for review hunk by hunk, in a pane beside it.
local s = ...
local fail = s.fail
local drafts = s.drafts
local agent_log = s.agent_log
local draft_one = s.draft_one
local said = s.said
local home = s.home
local prompts = s.prompts
local answers = s.answers

local reply = drafts .. "/2.eml"
local said_by = "On 1 Mar 2026, Bob <bob@example.com> wrote:"
vim.fn.writefile(
	{ "To: bob@example.com", "Subject: Re: Hi", "", "hi bob", "", "see you", "", said_by, "> Shall we meet?" },
	reply
)
vim.cmd.edit(reply)
local draft_buf = vim.api.nvim_get_current_buf()
-- The Quote folds away, the line naming whom it quotes in, the words above out
if vim.fn.foldclosed(8) ~= 8 or vim.fn.foldclosedend(8) ~= 9 or vim.fn.foldclosed(6) ~= -1 then
	fail(("the Quote folds %d to %d"):format(vim.fn.foldclosed(8), vim.fn.foldclosedend(8)))
end

-- A Quote the user opened stays open when the Draft shows again
vim.cmd("8foldopen")
vim.cmd.buffer(draft_one)
vim.cmd.buffer(draft_buf)
if vim.fn.foldclosed(8) ~= -1 then
	fail("the opened Quote folded again")
end

local function body()
	return vim.api.nvim_buf_get_lines(draft_buf, 2, -1, false)
end

-- Types `text` in the pane's input and sends it, as the user would.
local function ask(text)
	home()
	vim.cmd("PostalAgent")
	vim.api.nvim_buf_set_lines(0, -2, -1, false, { text })
	vim.cmd.normal(vim.keycode("<CR>"))
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
-- The agent upper-cases the Quote too, which Postal keeps as it was
turn({ "", "HI BOB", "", "SEE YOU", "", said_by, "> Shall we meet?" })
if vim.api.nvim_buf_get_lines(draft_buf, 0, 2, false)[1] ~= "To: bob@example.com" then
	fail("the headers changed")
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

if
	first[#first - 1].resource.text ~= "hi bob\n\nsee you\n\n" .. said_by .. "\n> Shall we meet?\n"
	or first[#first].text ~= "make it formal"
then
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
shows({ "hi bob", "see you" })
left("2 hunks left")
left("? keys")
-- A key set in setup() maps as given, and one set to false maps nothing
local pane_maps = {}
for _, map in ipairs(vim.api.nvim_buf_get_keymap(pane_buf, "n")) do
	pane_maps[map.lhs] = true
end

if not pane_maps["<C-Q>"] or pane_maps.q or pane_maps["<C-C>"] then
	fail("the pane maps " .. vim.inspect(vim.tbl_keys(pane_maps)))
end

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
is({ "", "HI BOB", "", "see you", "", said_by, "> Shall we meet?" })
shows({})
left("")

-- A hand edit goes with the next turn, which carries no rules again
vim.api.nvim_buf_set_lines(draft_buf, 3, 4, false, { "hello bob" })
vim.api.nvim_buf_set_lines(draft_buf, 6, 6, false, { "", "drop me" })
ask("shorter")
turn({ "", "HELLO BOB", "", "SEE YOU", "", said_by, "> Shall we meet?" })
shows({ "hello bob", "see you", "", "drop me" })
local second = prompts()[2].params.prompt
local asked = "hello bob\n\nsee you\n\ndrop me\n\n" .. said_by .. "\n> Shall we meet?\n"
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
is({ "", "hello bob", "", "see you", "", "drop me", "", said_by, "> Shall we meet?" })
shows({})
if not vim.wait(5000, function()
	return #prompts() == 3
end) then
	fail("the waiting instruction was never sent")
end

-- The user's edit while the agent works is never overwritten unasked
vim.api.nvim_buf_set_lines(draft_buf, 3, 4, false, { "hey bob" })
vim.fn.writefile({}, agent_log .. ".go")
turn({ "", "hey bob", "", "SEE YOU", "", said_by, "> Shall we meet?" })
shows({ "HELLO BOB", "see you", "", "drop me" })
left("2 hunks left (1 conflict)")
press(4, "<leader>a")
left("1 hunk left")
press(4, "<leader>A")
is({ "", "HELLO BOB", "", "SEE YOU", "", said_by, "> Shall we meet?" })
shows({})
left("")

-- An undo past the agent's edit ends its review, keeping the user's typing
-- before it, so no later decision writes over the wrong rows
vim.api.nvim_buf_set_lines(draft_buf, 3, 4, false, { "undo bob" })
ask("again")
turn({ "", "UNDO BOB", "", "SEE YOU", "", said_by, "> Shall we meet?" })
left("1 hunk left")
home()
vim.cmd.undo()
if not vim.wait(5000, function()
	return said(pane_buf):find("The agent's edit was undone", 1, true) ~= nil
end) then
	fail("the undo left the review open: " .. vim.inspect(said(pane_buf)))
end

is({ "", "undo bob", "", "SEE YOU", "", said_by, "> Shall we meet?" })
shows({})
left("")
if vim.fn.maparg("<leader>a", "n", false, true).buffer == 1 then
	fail("the review's keys outlived the undo")
end

-- An agent that ends with its edit under review hands the review to the
-- next one, whose first instruction waits for it
vim.api.nvim_buf_set_lines(draft_buf, 3, 4, false, { "bye bob" })
ask("quit")
turn({ "", "BYE BOB", "", "SEE YOU", "", said_by, "> Shall we meet?" })
if not vim.wait(5000, function()
	return said(pane_buf):find("stub ended", 1, true) ~= nil
end) then
	fail("the quitting agent never ended: " .. vim.inspect(said(pane_buf)))
end

local before_restart = #prompts()
ask("after")
vim.wait(500)
if #prompts() ~= before_restart then
	fail("the restarted chat prompted while a review was open")
end

left("1 hunk left")
press(4, "<leader>a")
is({ "", "BYE BOB", "", "SEE YOU", "", said_by, "> Shall we meet?" })
left("")
if not vim.wait(10000, function()
	return #prompts() == before_restart + 1
end) then
	fail("the restarted chat never sent the waiting instruction")
end

turn({ "", "BYE BOB", "", "SEE YOU", "", said_by, "> Shall we meet?" })

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
vim.cmd.normal(vim.keycode("<C-q>"))
if not vim.wait(5000, function()
	return said(pane_buf):find("Thinking no more.\n> Stopped.", 1, true) ~= nil
end) then
	fail("the turn was not stopped: " .. vim.inspect(said(pane_buf)))
end

local tail = vim.api.nvim_buf_get_lines(pane_buf, -4, -1, false)
if not vim.deep_equal(tail, { "", "## Me", "" }) or not said(pane_buf):find("## Me\n\nhang\n", 1, true) then
	fail("the input is not left empty at the bottom: " .. vim.inspect(said(pane_buf)))
end

s.draft_buf = draft_buf
s.pane_buf = pane_buf
