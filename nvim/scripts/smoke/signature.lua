local s = ...
local fail = s.fail
local said = s.said
local home = s.home
local prompts = s.prompts

-- A Signature is all body: the agent's edit to it starts at its first line,
-- for the same review as a Draft's
local signatures = vim.fn.tempname() .. "/postal/signatures"
vim.fn.mkdir(signatures, "p")
vim.fn.writefile({ "Ada Lovelace", "London" }, signatures .. "/1.txt")
home()
vim.cmd.edit(signatures .. "/1.txt")
local signature_buf = vim.api.nvim_get_current_buf()
local before_signature = #prompts()
-- A prompt set up by name is sent as the user's own; a name neither an
-- agent nor a prompt is refused, a default set to false among them, and
-- one for another kind of mail
for _, word in ipairs({ "formal", "summary" }) do
	vim.cmd("PostalAgent " .. word)
	if not (s.notified[#s.notified] or ""):find("no agent or prompt " .. word, 1, true) then
		fail("the prompt " .. word .. " passed over a Signature: " .. vim.inspect(s.notified))
	end
end

vim.cmd("PostalAgent stub shout")
local signature_pane = vim.api.nvim_get_current_buf()
local function signature()
	return vim.api.nvim_buf_get_lines(signature_buf, 0, -1, false)
end

if
	not vim.wait(10000, function()
		return vim.deep_equal(signature(), { "ADA LOVELACE", "LONDON" })
			and said(signature_pane):find("Signed.", 1, true) ~= nil
	end)
then
	fail("the Signature is " .. vim.inspect(signature()) .. " and its pane " .. vim.inspect(said(signature_pane)))
end

local signing = prompts()[before_signature + 1].params.prompt
if
	not signing[1].text:find("write_signature", 1, true)
	or signing[2].resource.text ~= "Ada Lovelace\nLondon\n"
	or signing[3].text ~= "Write it in capitals."
	or not said(signature_pane):find("## Me\n\nWrite it in capitals.\n", 1, true)
then
	fail("the Signature's prompt holds " .. vim.inspect(signing))
end

local reviewed = "✓ write_signature · the edit is in the Signature for review"
if not said(signature_pane):find(reviewed, 1, true) then
	fail("the pane lacks " .. vim.inspect(reviewed) .. " in " .. vim.inspect(said(signature_pane)))
end

vim.cmd("bwipeout! " .. signature_buf)
if vim.api.nvim_buf_is_valid(signature_pane) then
	fail("the pane outlived its Signature")
end
