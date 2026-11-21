-- Completes the recipients of a Postal Draft from its Correspondents:
-- through blink.cmp when it is installed, else `<C-x><C-o>`.
local recipients = require("postal.recipients")

local M = {}

-- Where Postal keeps its Drafts, whatever `$XDG_DATA_HOME` is.
local DRAFTS = "*/postal/drafts/*.eml"

--- `opts.cmd` is the command Postal runs as, `postal` by default.
function M.setup(opts)
  opts = opts or {}
  recipients.cmd = opts.cmd or recipients.cmd
  local group = vim.api.nvim_create_augroup("postal", { clear = true })
  vim.api.nvim_create_autocmd({ "BufReadPost", "BufNewFile" }, {
    group = group,
    pattern = DRAFTS,
    callback = function(event)
      vim.b[event.buf].postal_draft = true
      vim.bo[event.buf].omnifunc = "v:lua.require'postal'.omnifunc"
      M.blink()
    end,
  })

  vim.api.nvim_create_autocmd("BufWipeout", {
    group = group,
    pattern = DRAFTS,
    callback = function(event)
      recipients.forget(event.buf)
    end,
  })
end

-- Registers the source with blink.cmp once, on the first Draft, so blink
-- loads no earlier than it would.
local registered = false
function M.blink()
  local ok, blink = pcall(require, "blink.cmp")
  if registered or not ok then
    return
  end

  registered = true
  blink.add_source_provider("postal", { name = "Postal", module = "postal.blink" })
  blink.add_filetype_source("mail", "postal")
end

--- `:h complete-functions` over the recipient under the cursor.
function M.omnifunc(findstart, base)
  if findstart == 1 then
    local row, col = unpack(vim.api.nvim_win_get_cursor(0))
    -- -3 leaves completion without an error message.
    return recipients.start(0, row, col) or -3
  end

  local items = {}
  for _, correspondent in ipairs(recipients.list(0)) do
    if recipients.matches(correspondent, base) then
      table.insert(items, { word = correspondent.mailbox, menu = correspondent.account })
    end
  end

  return items
end

return M
