-- The recipients of a Postal Draft: where one starts under the cursor, and
-- the Correspondents `postal correspondent list` offers for it.
local M = {}

--- The command Postal runs as; `setup` may point it at a build.
M.cmd = "postal"

-- Each Draft's Correspondents, asked for once per buffer.
local lists = {}

--- The 0-based byte column where the recipient under the cursor starts, in
--- the `To` or `Cc` header of buffer `buf` or a line folded into it; nil
--- anywhere else. `row` is 1-based and `col` a 0-based byte column.
function M.start(buf, row, col)
  local lines = vim.api.nvim_buf_get_lines(buf, 0, row, false)
  for n = 1, row do
    if lines[n] == "" then
      return nil
    end
  end

  local header = row
  while header >= 1 and lines[header]:match("^[ \t]") do
    header = header - 1
  end

  local name = header >= 1 and lines[header]:match("^([^:%s]+):")
  if not name or not vim.tbl_contains({ "to", "cc" }, name:lower()) then
    return nil
  end

  local before = lines[row]:sub(1, col)
  local start = header == row and #name + 1 or 0
  -- A comma inside quotes or angle brackets is part of one recipient.
  local quoted, angled = false, false
  for n = start + 1, #before do
    local c = before:sub(n, n)
    if c == '"' then
      quoted = not quoted
    elseif c == "<" and not quoted then
      angled = true
    elseif c == ">" and not quoted then
      angled = false
    elseif c == "," and not quoted and not angled then
      start = n
    end
  end

  while start < col and before:sub(start + 1, start + 1):match("%s") do
    start = start + 1
  end

  return start
end

--- The Correspondents for buffer `buf`'s Draft, best first, each with
--- `address`, `name`, `mailbox`, `account` and `written`; empty when Postal
--- cannot answer, which it says once.
function M.list(buf)
  if lists[buf] then
    return lists[buf]
  end

  local path = vim.api.nvim_buf_get_name(buf)
  local command = { M.cmd, "correspondent", "list", "--json", "--draft", path }
  local ok, result = pcall(function()
    return vim.system(command, { text = true }):wait()
  end)

  local list = {}
  if not ok then
    vim.notify_once("postal: " .. tostring(result), vim.log.levels.WARN)
  elseif result.code ~= 0 then
    vim.notify_once(vim.trim(result.stderr), vim.log.levels.WARN)
  else
    list = vim.json.decode(result.stdout, { luanil = { object = true } })
  end

  lists[buf] = list
  return list
end

function M.forget(buf)
  lists[buf] = nil
end

--- Whether each word of `query` is found in the Correspondent's name or
--- address, case aside.
function M.matches(correspondent, query)
  local haystack = vim.fn.tolower((correspondent.name or "") .. " " .. correspondent.address)
  for word in vim.fn.tolower(query):gmatch("%S+") do
    if not haystack:find(word, 1, true) then
      return false
    end
  end

  return true
end

return M
