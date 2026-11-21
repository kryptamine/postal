# The Neovim plugin is an ACP client of its own, and the agent edits only a Draft's body

Postal's main use of an AI agent is writing a Reply together with it: the
Draft stays open in Neovim, and a chat pane beside it shows what the agent
says. All of it lives in the plugin. Postal itself holds no model, no key
and no agent; it hands out context through the CLI (ADR 0017), and mail
leaves the machine only once the user opens the pane on a Draft and asks.

The plugin speaks the Agent Client Protocol (JSON-RPC 2.0 over the agent's
stdio, one message per line) in its own few hundred lines, with no other
plugin beneath it. Any agent that speaks ACP plugs in by its command in
`setup()`: Claude Code through `claude-agent-acp`, Codex, Gemini. Each
Draft buffer gets its own agent process and session, ended when the buffer
is unloaded. The agent runs in an empty directory of its own, so tools it
runs without asking, such as a search, find nothing on disk.

The agent signs in with its own CLI. When `session/new` or a prompt answers
`auth_required`, the pane shows the sign-in the agent's `authMethods`
describe and nothing more: the plugin never calls `authenticate` and never
sees a secret.

The first prompt carries the Draft's rules (edit the body only, keep the
quote, never send) and the Thread `postal thread show --draft` prints, as an
embedded resource; every prompt carries the body as it stands, the user's
own edits included, and the instruction; to an agent without
`embeddedContext` the resources go as plain text. An instruction given
while a turn runs waits for it to end.

The agent sees the Draft's file as its body alone: the client offers
`fs/read_text_file` and `fs/write_text_file` for that one path, reading the
buffer's body and offering a write for review hunk by hunk (ADR 0019), so
an agent that edits through the client cannot touch the headers, whatever
the rules say.
Any other path is refused, there is no terminal, and every permission
request is denied but an edit whose locations are all the Draft, allowed
once each time, never always. A write lands in the buffer, never on disk;
saving stays the user's, and so does a Send.

## Considered options

- **An existing ACP plugin (CodeCompanion, avante.nvim).** A large
  dependency for one pane, and its own idea of which tools an agent may
  use; the rules above would be its settings, not Postal's.
- **The whole Draft file to the agent.** Simpler to describe, but the
  headers become the agent's to rewrite, and a write has to be split back
  into headers and body by guessing where one ends.
- **The agent writing the file on disk.** The user's unsaved edits would
  conflict with it, and undo would not reach it.
