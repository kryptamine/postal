local s = ...
local fail = s.fail
local drafts = s.drafts

-- A Draft Postal sends as Markdown parses its own text as Markdown, and
-- neither its headers, its signature nor the Quote; it stays a Draft
vim.env.POSTAL_COMPOSE_FORMAT = "markdown"
local markdown_draft = drafts .. "/markdown.eml"
vim.fn.writefile({
	"To: bob@example.com",
	"Subject: Plan",
	"",
	"# Plan",
	"- **first**",
	"",
	"-- ",
	"Bob",
	"",
	"On Mon, Ada wrote:",
	"> # not mine",
}, markdown_draft)
vim.env.POSTAL_SEND_STATUS = "100"
vim.cmd.split(markdown_draft)
vim.env.POSTAL_COMPOSE_FORMAT = nil
vim.env.POSTAL_SEND_STATUS = nil
if vim.fn.exists(":PostalSend") ~= 2 then
	fail("no :PostalSend in a Draft Postal opened")
end

local parser = vim.treesitter.get_parser(0, nil, { error = false })
local first, last
if parser then
	first, _, last = parser:parse(true)[1]:root():range()
end

if
	vim.bo.filetype ~= "mail.markdown"
	or vim.bo.omnifunc ~= "v:lua.require'postal'.omnifunc"
	or vim.bo.syntax ~= "mail"
	or vim.wo.conceallevel ~= 2
	or first ~= 3
	or last ~= 6
then
	fail(
		("the Markdown Draft has filetype=%s omnifunc=%s syntax=%s conceallevel=%d and parses rows %s to %s"):format(
			vim.bo.filetype,
			vim.bo.omnifunc,
			vim.bo.syntax,
			vim.wo.conceallevel,
			first,
			last
		)
	)
end

-- An edit above the signature moves where the Markdown ends
vim.api.nvim_buf_set_lines(0, 5, 5, false, { "more" })
vim.api.nvim_exec_autocmds("TextChanged", { buffer = 0 })
local _, _, moved = parser:parse(true)[1]:root():range()
if moved ~= 7 then
	fail("after an edit the Markdown ends at row " .. tostring(moved))
end

vim.cmd("bwipeout!")
