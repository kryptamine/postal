-- The hunks of an agent's edit to a body, set against the user's own
-- edits to it since the turn was sent: those that touch are conflicts.
local M = {}

local function split(text)
	if text == "" then
		return {}
	end

	return vim.split((text:gsub("\n$", "")), "\n", { plain = true })
end

-- vim.diff's hunks as 0-based ranges, `[a, a + ca)` of `old` and
-- `[b, b + cb)` of `new`; an insertion's `start` is the line before it.
local function diff(old, new)
	local hunks = {}
	for _, hunk in ipairs(vim.diff(old, new, { result_type = "indices" })) do
		local a, ca, b, cb = unpack(hunk)
		table.insert(hunks, { a = ca == 0 and a or a - 1, ca = ca, b = cb == 0 and b or b - 1, cb = cb })
	end

	return hunks
end

-- Where `[lo, hi)` of the old side lies on the new side of `hunks`, every
-- hunk touching the range being inside it.
local function span(hunks, lo, hi)
	local start, stop = lo, hi
	for _, hunk in ipairs(hunks) do
		if hunk.a + hunk.ca < lo then
			start = start + hunk.cb - hunk.ca
			stop = stop + hunk.cb - hunk.ca
		elseif hunk.a <= hi then
			stop = stop + hunk.cb - hunk.ca
		end
	end

	return start, stop
end

-- Runs of the agent's hunks, each joined with every hunk of the user's
-- that touches it, adjacent lines included, as git counts a conflict.
local function runs(agent, user)
	local items = {}
	for _, hunk in ipairs(agent) do
		table.insert(items, { lo = hunk.a, hi = hunk.a + hunk.ca, agent = true })
	end

	for _, hunk in ipairs(user) do
		table.insert(items, { lo = hunk.a, hi = hunk.a + hunk.ca })
	end

	table.sort(items, function(x, y)
		return x.lo < y.lo
	end)

	local joined = {}
	for _, item in ipairs(items) do
		local run = joined[#joined]
		if run and item.lo <= run.hi and (run.agent ~= item.agent or run.conflict) then
			run.hi = math.max(run.hi, item.hi)
			run.conflict = true
			run.agent = run.agent or item.agent
		else
			table.insert(joined, { lo = item.lo, hi = item.hi, agent = item.agent })
		end
	end

	return vim.tbl_filter(function(run)
		return run.agent
	end, joined)
end

--- The hunks between body `now` and the agent's `proposed`, both grown
--- from `sent`, top to bottom: `at` is the 0-based line of `now` where
--- `old`, its lines, would become `new`. A conflict's `old` is the user's.
function M.between(sent, now, proposed)
	local agent, user = diff(sent, proposed), diff(sent, now)
	local proposed_lines, now_lines = split(proposed), split(now)
	local hunks = {}
	for _, run in ipairs(runs(agent, user)) do
		local new_start, new_stop = span(agent, run.lo, run.hi)
		local now_start, now_stop = span(user, run.lo, run.hi)
		local new = vim.list_slice(proposed_lines, new_start + 1, new_stop)
		local old = vim.list_slice(now_lines, now_start + 1, now_stop)
		-- The user wrote what the agent did: nothing to ask.
		if not vim.deep_equal(new, old) then
			table.insert(hunks, { at = now_start, old = old, new = new, conflict = run.conflict == true })
		end
	end

	return hunks
end

return M
