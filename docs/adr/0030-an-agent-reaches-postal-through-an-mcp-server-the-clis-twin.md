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

A command's arguments are one struct, which clap parses and the tool's
schema is derived from; `--json` is a flag of `postal` itself, so no tool
takes it. One table lists the commands, and each tool's name, description
and schema come from its command's path, help and struct. A tool answers
both ways: the text the command prints, and the `--json` array under
`items` in `structuredContent`, which MCP wants an object. The server is
`rmcp`, the protocol's own Rust SDK, so Postal keeps no JSON-RPC of its own.

## How a command names mail

A listing gives each Thread and Message an ID that another command takes:
`m`, then a tag of the Account and the Message's body key (ADR 0005); `t`,
then whether Gmail's Thread, the root of `References` or a lone Message
names it (ADR 0009), the tag and that key. The tag is 20 bits of the
address's hash, and each part is Crockford's base32, so an ID is 18 or 19
characters an agent copies as they are.

Nothing is stored: an ID is read back into the key, and the key looked up.
It lives as long as the key does: on Gmail for good, elsewhere until the
Folder's UIDVALIDITY changes. A Search from the CLI finds what the TUI's
does, but lists it newest first: nucleo's ranking lives in the TUI.

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
- **A hash of the key as the ID.** Ten characters instead of 18, but a
  lookup hashes every row of the Cache, and two keys may share one.
- **An ID stored in the Cache.** Shortest, but only a sync could assign
  it, a migration would add it, and it would last no longer than the key.
- **`Message-ID` as the ID.** It outlives every Cache, but is long, can be
  missing or repeated, and the Cache does not keep it.
- **A remote MCP server over HTTP**, for claude.ai and phones. The mail and
  its credentials would live on a host, and mail would leave the machine
  without the user asking, against ADR 0018. Not now.
