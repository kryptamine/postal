# An agent reaches Postal through an MCP server, the CLI's twin

An agent should work with mail wherever it runs, not only in the Neovim
pane of ADR 0018: in Claude Code, Codex, Gemini, Claude Desktop or an
editor's chat. The CLI (ADR 0017) already serves one through its shell, and
issue #69 grows it. A shell is the wrong boundary to stop at, for three
reasons:

- Many agents have none, or one the user keeps switched off.
- A shell is allowed whole or not at all. Telling `thread show` from a send
  means trusting a parse of the command's text, as ADR 0020 found.
- Mail is written by anyone. An agent that reads it and can act on it is
  open to prompt injection, so each action has to be its own permission,
  and a send has to stop for the user.

So `postal mcp` runs an MCP server on its stdio, in the binary itself. Any
MCP client starts it by that command. Each tool is one operation with a
typed schema, and the client asks the user tool by tool.

## One set of operations, two surfaces

The CLI and the MCP server are two adapters over the same operations: the
logic lives once, below both, and neither holds any of its own. They match
one to one. `postal thread show` is the tool `thread_show`, its flags are
the tool's arguments, and its `--json` output is the tool's result, with
the same fields, which only grow. A command lands with its tool in the same
change, so an agent that knows one surface knows the other, and a test
through either covers the operation.

The CLI stays for people and scripts. The MCP server is the way in for an
agent. A skill may sit above the tools to teach a way of working, such as
triaging an Inbox, but never to teach the commands: their schemas do that.

## Reads first

The server starts with reads only (issue #144), as the CLI does: it opens
the Cache read-only, never syncs and never writes, runs beside a TUI
already open, and refuses a missing Cache or one of another schema. It
knows what the TUI or a `postal sync` (issue #69) last synced.

Writes come later, each in its own decision: a Move, an Archive, a flag, a
Draft. Two things hold for them already:

- A send is never a tool the agent calls alone. The agent writes a Draft,
  and the user sends it, or confirms it when the server asks (MCP
  elicitation).
- A write with a TUI open is the question issue #69 asks of `postal sync`:
  two writers on one Cache. The answer serves both surfaces.

The plugin's MCP server (ADR 0020) stays as it is: it edits a Neovim
buffer, which `postal mcp` cannot reach. An agent in the pane may get both.

## Considered options

- **A skill over the CLI alone.** No code beyond the CLI, and pipes
  compose. But it needs a shell, permissions are all or nothing, and
  nothing stops a send for the user.
- **An MCP server with no CLI twin.** One surface less to keep, but a
  person and a script lose the commands, and the tests lose the simpler
  boundary.
- **A remote MCP server over HTTP**, for claude.ai and phones. The mail and
  its credentials would live on a host, and mail would leave the machine
  without the user asking, against ADR 0018. Not now.
