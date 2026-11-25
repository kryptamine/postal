# A launcher app hands mailto: links to Postal

`postal setup-mailto` makes Postal the system's `mailto:` handler (issue
#190): a mail link clicked in a browser opens a terminal running `postal
<link>` (ADR 0033 covers what that does beside an open TUI).

## macOS

A command-line program never receives a URL: LaunchServices sends it as an
Apple Event to an app whose `Info.plist` names the scheme. Postal therefore
carries a small app, `macos/launcher.swift`, `Postal.app` with the bundle
identifier `dev.postalmail.mailto`.

**Built at release, carried in the binary.** `make launcher` compiles it
with `swiftc`, and `make dist` signs and notarizes it with the binary's
Developer ID, staples it and zips it; `build.rs` takes the zip
`POSTAL_LAUNCHER` names into the binary. So no user needs Xcode or `swiftc`,
and the archive and the install script stay one file. `postal update`
leaves the installed app as it is; `setup-mailto` run again replaces it with
the one the binary carries. A build without it, as `cargo build`
leaves, says so on `setup-mailto`; `make install` builds it on a Mac.

**Installed by setup.** `setup-mailto` unpacks it into `~/Applications`,
then runs `Postal.app/Contents/MacOS/postal-mailto register <postal>`. The
app keeps the path of `postal` and the handler it replaces in its own user
defaults, and makes itself the default through
`NSWorkspace.setDefaultApplication`, which may ask the user to confirm.
`--remove` runs `unregister`, which gives the scheme back to the handler
kept, Mail without one, then deletes the app.

**One argument, no shell.** On a link, the app runs `postal
open-in-terminal <link>` with `Process`: the link is one argv element, read
from the event as a string, as a `URL` would re-encode it. Postal reads
`terminal` from `config.toml` and starts it with `open -na <app> --args`,
the link again one element: Ghostty with `-e`, kitty as its program,
WezTerm with `start --`. `terminal = { command = "..." }` names a program of
the user's own in its place, run with the path of `postal` and the link; it
is one path, never split into words, so a wrapper script holds any flags the
terminal needs. Postal never puts the link in a script, where a `'` in a
link from a web page would run a command.

**Terminal.app and iTerm2** take no command as arguments, only a file to
run, or an AppleScript `do script`, whose text a shell parses. Each link
gets a directory of its own, mode 0700, under
`$XDG_STATE_HOME/postal/mailto/`: the link and the path of `postal` in two
files, and `postal.command`, the same fixed script every time. The script
reads both files with `"$(cat ...)"`, which a shell takes as one word and
never parses again, removes the directory, then execs `postal <link>`. So a
shell sees the link only as data, each link opens once, and the files go
with it. A directory whose terminal never started stays behind.

## Linux

`$XDG_DATA_HOME/applications/postal.desktop` runs `"<postal>" %u` with
`Terminal=true`, the path quoted as the Desktop Entry spec asks, then
`xdg-mime default postal.desktop x-scheme-handler/mailto`. The desktop picks
the terminal, so Settings offers none. `--remove` deletes the entry, and
the desktop falls back to the next handler it knows: `xdg-mime` and GIO skip
a default whose entry is gone.

## Where it lives

The handler is `account::mailto_handler`, beside `xdg`, as it imports only
`account`: `handler_app` on macOS, `handler_desktop` elsewhere, the other
compiled too so either system lints both. `CHOOSES_TERMINAL` tells Settings
whether to list terminals, so the screen asks the handler, not the system.

## Settings

The Mailto tab switches the handler, as `setup-mailto` and its `--remove`
do, and on macOS chooses the terminal or types the command.
