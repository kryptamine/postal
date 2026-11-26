local s = ...
local fail = s.fail
local sent = s.sent
local draft_buf = s.draft_buf
local pane_buf = s.pane_buf

-- The session ends with its Draft's buffer
vim.cmd("bwipeout! " .. draft_buf)
if vim.api.nvim_buf_is_valid(pane_buf) then
	fail("the pane outlived its Draft")
end

-- The chat's agent is the one started after `quit`, the second to log.
local pid = vim.tbl_filter(function(message)
	return message.pid ~= nil
end, sent())[2].pid
if not vim.wait(5000, function()
	return vim.uv.kill(pid, 0) ~= 0
end) then
	fail("the agent outlived its Draft")
end
