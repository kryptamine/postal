# Demo

`make demo` runs Postal against the fake IMAP and SMTP servers of the tests,
seeded with a mailbox of invented people and companies. Every screenshot and
video of the landing page is shot from it, so a frame never shows real mail
or another company's brand, and each can be shot again when the UI changes.

The run touches nothing of yours: config, secrets, Cache, Drafts and the
Neovim it opens for Compose all live in a temporary directory, removed
when Postal quits.

- `main.rs` starts the servers, writes the home and runs `postal`.
- `mailbox/` is the mail: a work Account at a Gmail-like server and a
  personal one at an iCloud-like server, for All Inboxes. Dates count back
  from the start of the run.
- `mail/` holds the HTML mails, written as MJML renders them, with `@DATE@`
  for the date of the run.
- `init.lua` is the Neovim of the run: the plugin of this checkout and the
  terminal's own palette.
- `agent.lua` is the ACP agent `:PostalAgent` starts: whatever it is asked,
  it rewrites the Draft the same way, so every take reviews the same hunks.

## Videos

`make media` records each tape of `tapes/` with
[VHS](https://github.com/charmbracelet/vhs) into `landing/public/media/`,
as MP4 and WebM. `setup.tape` is shared: the palette and font of the
stills, and the demo started off camera, with `open` stubbed so a link
opened on camera starts nothing. A tape shows one action in 5 to 11
seconds; each starts from the same mailbox, so a new take replaces the
old one whole.
