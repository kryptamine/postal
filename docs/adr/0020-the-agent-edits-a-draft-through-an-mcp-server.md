# The agent reads and writes a Draft through an MCP server of the plugin's own

ADR 0018 had the agent edit the Draft through the client's
`fs/read_text_file` and `fs/write_text_file`. `claude-agent-acp` never
calls them, in 0.23 or in 0.109: Claude's Read, Edit and Write go straight
to disk, so each asks for a permission the plugin denies, and the agent is
left with no way to reach the Draft. Allowing them would show it the
headers and write past the hunk review of ADR 0019.

So the plugin hands every session, in `session/new`'s `mcpServers`, a stdio
MCP server of its own: `nvim -l lua/postal/mcp.lua <servername> <buffer>`.
It serves two tools, and passes each call to the editor over its
`v:servername`:

- `read_draft` gives the body as it stands, the user's unsaved edits
  included, or the agent's last write in this turn.
- `write_draft` takes the whole new body and opens the hunk review.

The headers never leave the editor, and a write lands in the buffer, never
on disk. The client offers no `fs/*` any more, and the body goes in each
prompt as a `postal-draft://` resource rather than a `file://` one, which
would invite a Read of the file.

A permission is granted only to those two tools, recognised by their exact
name: `_meta.claudeCode.toolName` where the agent sends it (0.109), else
the title, which an older `claude-agent-acp` sets to the tool's name. A
title that only mentions one, such as a shell command, does not match.
Every other request, Read of the Draft's own file included, stays denied.

## Considered options

- **Allow Claude's Read and Edit of the Draft's file, and watch the
  file.** The agent would see the headers, and its write would race the
  user's unsaved edits before any review.
- **Postal CLI commands the agent runs through its shell.** A write would
  go to disk past the buffer, and allowing a shell command means trusting
  a parse of its text.
- **The new body in the agent's reply, cut out by the plugin.** Works with
  any agent, but rests on the model keeping to a format.
