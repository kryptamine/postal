-- What the user sent to an agent, oldest first, shared by every chat until
-- Neovim exits.
local M = {}

M.sent = {}

function M.add(text)
	if M.sent[#M.sent] ~= text then
		table.insert(M.sent, text)
	end
end

return M
