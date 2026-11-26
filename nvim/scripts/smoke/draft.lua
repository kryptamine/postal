-- A Draft completes its recipients and `Attach:` paths, and its checks
-- mark what a send would refuse or regret.
local s = ...
local fail = s.fail
local drafts = s.drafts
local diagnosed = s.diagnosed

local draft = drafts .. "/1.eml"
vim.fn.writefile({
	"To: bob@example.com",
	"Cc: Ada Lovelace <ada@example.com>, pet",
	"Subject: Hi",
	"",
	"pet",
}, draft)
vim.cmd.edit(draft)

-- The ftplugin's `textwidth` would cut each paragraph into hard lines
if vim.bo.textwidth ~= 0 or vim.bo.formatoptions:find("t", 1, true) or not vim.wo.spell then
	fail(("the Draft has textwidth=%d formatoptions=%s"):format(vim.bo.textwidth, vim.bo.formatoptions))
end

-- Postal alone waits for `:PostalSend`'s exit, and says so in the env
if vim.fn.exists(":PostalSend") == 2 then
	fail(":PostalSend without $POSTAL_SEND_STATUS")
end

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

-- A recipient a send would refuse is an error before the send, and one no
-- mail holds says so once the Correspondents are in
local want_diagnostics = {
	{ 0, 4, vim.diagnostic.severity.INFO, "no mail of the accounts is from or to bob@example.com" },
	{ 1, #"Cc: Ada Lovelace <ada@example.com>, ", vim.diagnostic.severity.ERROR, "`pet` is not an address" },
}

if not vim.wait(5000, function()
	return vim.deep_equal(diagnosed("address"), want_diagnostics)
end) then
	fail("the recipients are diagnosed " .. vim.inspect(diagnosed("address")))
end

vim.api.nvim_buf_set_lines(0, 1, 2, false, { 'Cc: "Ada <ada@example.com>, petr@example.com' })
vim.api.nvim_exec_autocmds("TextChanged", { buffer = 0 })
if diagnosed("address")[2][4] ~= "a quote is not closed" then
	fail("an open quote is diagnosed " .. vim.inspect(diagnosed("address")))
end

-- A recipient folded over two lines is one, as Postal joins them
vim.api.nvim_buf_set_lines(0, 1, 2, false, { "Cc: Ada Lovelace", "  <ada@example.com>" })
vim.api.nvim_exec_autocmds("TextChanged", { buffer = 0 })
if #diagnosed("address") ~= 1 then
	fail("a folded recipient is diagnosed " .. vim.inspect(diagnosed("address")))
end

vim.api.nvim_buf_set_lines(0, 1, 3, false, { "Cc: Ada Lovelace <ada@example.com>, pet" })

-- :PostalContacts adds the Correspondent picked to the header under the
-- cursor, an empty one included, and to To: from the body
local ui_select = vim.ui.select
local picked = {}
vim.ui.select = function(items, opts, choose)
	table.insert(picked, opts.format_item(items[1]))
	choose(items[1])
end

vim.api.nvim_buf_set_lines(0, 1, 2, false, { "Cc: " })
vim.api.nvim_buf_set_lines(0, 0, 1, false, { "To: bob@example.com, " })
for _, row in ipairs({ 2, 2, 5 }) do
	vim.api.nvim_win_set_cursor(0, { row, 0 })
	vim.cmd("PostalContacts")
end

vim.ui.select = ui_select
local petr = "Petr Novak <petr@example.com>"
local headers = vim.api.nvim_buf_get_lines(0, 0, 2, false)
if not vim.deep_equal(headers, { "To: bob@example.com, " .. petr, "Cc: " .. petr .. ", " .. petr }) then
	fail(":PostalContacts made the headers " .. vim.inspect(headers))
end

if picked[1] ~= petr .. "  me@example.com" then
	fail(":PostalContacts offered " .. vim.inspect(picked))
end

vim.api.nvim_buf_set_lines(0, 0, 2, false, { "To: bob@example.com", "Cc: Ada Lovelace <ada@example.com>, pet" })

-- `Attach:` completes as Postal reads it: a relative path is under $HOME
local real_home = vim.env.HOME
vim.env.HOME = vim.fn.tempname()
vim.fn.mkdir(vim.env.HOME .. "/Documents", "p")
vim.fn.writefile({}, vim.env.HOME .. "/Documents/report.pdf")
for typed, want_path in pairs({ ["Documents/rep"] = "Documents/report.pdf", ["~/Doc"] = "~/Documents/" }) do
	vim.api.nvim_buf_set_lines(0, 3, 3, false, { "Attach: " .. typed })
	local path_start, paths = at(4, #"Attach: " + #typed)
	vim.api.nvim_buf_set_lines(0, 3, 4, false, {})
	if path_start ~= #"Attach: " or not vim.deep_equal(paths, { want_path }) then
		fail("`Attach: " .. typed .. "` offered " .. vim.inspect(paths) .. " from " .. path_start)
	end
end

-- Each `Attach:` line shows its file's size or what is wrong with it, and
-- the files over Postal's limit together say so before a send refuses them;
-- a folded header is read whole, as Postal reads it
local big = vim.uv.fs_open(vim.env.HOME .. "/big.bin", "w", 420)
vim.uv.fs_ftruncate(big, 26 * 1024 * 1024)
vim.uv.fs_close(big)
local draft_one = vim.api.nvim_get_current_buf()
vim.api.nvim_buf_set_lines(
	0,
	3,
	3,
	false,
	{ "Attach: Documents/report.pdf", "Attach: gone.txt", "attach:", "  ~/big.bin" }
)
vim.api.nvim_exec_autocmds("TextChanged", { buffer = 0 })
local marked = {}
local attach_ns = vim.api.nvim_get_namespaces()["postal-attach"]
for _, mark in ipairs(vim.api.nvim_buf_get_extmarks(0, attach_ns, 0, -1, { details = true })) do
	marked[#marked + 1] = mark[2] .. " " .. mark[4].virt_text[1][1]
end

local want_marks = { "3 0 B", "4 no such file", "5 26.0 MB", "5 26.0 MB together, over 25 MB" }
if not vim.deep_equal(marked, want_marks) then
	fail("the Attach: lines are marked " .. vim.inspect(marked))
end

-- `gf` edits the file as Postal reads the path, and on a missing one stays
local function gf(row)
	vim.api.nvim_win_set_cursor(0, { row, 0 })
	vim.cmd.normal("gf")
end

gf(4)
if vim.uv.fs_realpath(vim.api.nvim_buf_get_name(0)) ~= vim.uv.fs_realpath(vim.env.HOME .. "/Documents/report.pdf") then
	fail("gf opened " .. vim.api.nvim_buf_get_name(0))
end

vim.cmd.buffer(draft_one)
gf(5)
if vim.api.nvim_get_current_buf() ~= draft_one then
	fail("gf left the Draft for a missing file")
end

vim.api.nvim_buf_set_lines(0, 3, 7, false, {})
vim.env.HOME = real_home

-- Words that mention an attachment warn while no Attach: names a file; the
-- Quote's are someone else's
local function mentioned(lines)
	vim.api.nvim_buf_set_lines(0, 3, -1, false, lines)
	vim.api.nvim_exec_autocmds("TextChanged", { buffer = 0 })
	return diagnosed("mention")
end

local warned = mentioned({ "", "pet", "The report is Attached." })
local want_warning = { 5, 14, vim.diagnostic.severity.WARN, "mentions an attachment, and no Attach: line names a file" }
if not vim.deep_equal(warned, { want_warning }) then
	fail("the mention is diagnosed " .. vim.inspect(warned))
end

if #mentioned({ "Attach: report.pdf", "", "The report is attached." }) ~= 0 then
	fail("a mention warned with a file attached")
end

if #mentioned({ "", "pet", "", "On 1 Mar 2026, Bob <bob@example.com> wrote:", "> see the attachment" }) ~= 0 then
	fail("a mention in the Quote warned")
end

mentioned({ "", "pet" })

-- A Draft to nobody is an error before the send, and one with no Subject
-- a warning; a Cc alone addresses it
local function blank(lines)
	vim.api.nvim_buf_set_lines(0, 0, 3, false, lines)
	vim.api.nvim_exec_autocmds("TextChanged", { buffer = 0 })
	return diagnosed("blank")
end

local found = blank({ "To: ", "Cc: ", "Subject:  " })
local want_blank = {
	{ 0, 0, vim.diagnostic.severity.ERROR, "no recipient in To: or Cc:" },
	{ 2, 0, vim.diagnostic.severity.WARN, "the Subject is empty" },
}
if not vim.deep_equal(found, want_blank) then
	fail("the empty headers are diagnosed " .. vim.inspect(found))
end

if #blank({ "To:", "Cc: ada@example.com", "Subject: Hi" }) ~= 0 then
	fail("a Draft to its Cc alone is diagnosed " .. vim.inspect(diagnosed("blank")))
end

blank({ "To: bob@example.com", "Cc: Ada Lovelace <ada@example.com>, pet", "Subject: Hi" })

s.draft_one = draft_one
