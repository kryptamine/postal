# Security Policy

## Supported versions

Postal is pre-1.0: a fix lands on `main`, and only the latest commit is
supported.

## Reporting a vulnerability

Email **satretdinovalexander@gmail.com** with `postal` in the subject. Do not
open a public issue for anything exploitable until a fix is out.

Include your Postal commit (`git rev-parse HEAD`), your OS and terminal, what
an attacker controls (a mail, a header, an attachment name) and the shortest
sequence that shows it.

You get an acknowledgement within 72 hours and an assessment within a week.
Tell me if and when you plan to disclose, and you get credit in the commit
unless you would rather not be named.

## Scope

Every mail is hostile input. Worth reporting:

- a mail that reaches the terminal with escape or control sequences, runs a
  command, or writes outside Postal's own directories;
- a mail that makes Postal fetch anything, such as a remote image;
- a crash, or unbounded memory or time, a mail can trigger on its own;
- a secret leaving the keychain or `config.toml` for anywhere but its server;
- the Neovim plugin's agent reaching more than its `read_*` and `write_*`
  tools: headers, commands or sending.

Not this project's to fix:

- vulnerabilities in your mail provider, terminal or ACP agent;
- anything that presupposes an attacker who already runs code as you;
- what the agent's provider does with the text you ask it about.
