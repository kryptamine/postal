---
name: landing-media
description: Shoot or re-shoot the landing page's screenshots and videos from the demo mailbox (make demo, make media, make stills). Use when the UI changed and the landing media is stale, when a new still or video is wanted, or when a tape or the stills script breaks.
---

# Landing media

How the files fit is in `demo/README.md`; read it first. This is the way
of working that produced them, and the traps on the way.

## Loop

1. `cargo build --release --bins --example demo`.
2. Explore before writing a tape: a scratch tape in `target/peek/` that
   takes `Screenshot`s after each key, then read the PNGs. Only commit a
   tape once every step shows what the slot on the page describes
   (`landing/src/pages/index.astro`, `shows=`).
3. Record with `vhs demo/tapes/<name>.tape`, then check frames, not the
   whole video: `ffmpeg -v error -y -ss 4 -i landing/public/media/x.mp4
   -frames:v 1 target/peek/x.png` and `-sseof -0.3` for the last frame;
   `ffprobe ... format=duration` for length (aim 5 to 11 s).
4. `make stills` for the screenshots (macOS, the user's own Ghostty: their
   theme and font, never changed). Preview with `sips -Z 1400`.
5. `make landing`, then look at the page: `pnpm preview` in `landing/` and
   a headless Chrome `--screenshot` of it.
6. Commit per layer (mailbox, tapes and videos, stills, landing wiring);
   ask before pushing: the push publishes the media.

## Traps

- VHS rejects paths starting with `/` in `Output` and `Screenshot`: use
  paths relative to the repo root. A `Screenshot` as the last command is
  dropped; end with a `Sleep`.
- Start a tape after `Wait+Screen@30s /Brightfold/` plus a short `Sleep`,
  or the two Accounts' syncs reorder the list under the keys.
- Neovim's recipient menu needs human pace: `Set TypingSpeed 260ms` while
  typing in `To:`. The agent pane's input starts in normal mode: `i`,
  text, `Escape`, `Enter`. Hunks accept with `\a` on their first line.
- Hint mode labels only real links: plain-text URLs get none, so use an
  HTML mail.
- The fake IMAP server panics on a date with sub-second precision; demo
  dates go through `to_rfc3339_opts(SecondsFormat::Secs, ..)`.
- The demo sets its own `HOME`, so an `Attach:` completion never shows the
  user's files. Keep it that way; add files in `demo/home.rs`.
- A screenshot of the real screen can catch the user's private windows:
  capture only the demo's window (`screencapture -l <id>`), and delete any
  test capture at once.
- Ghostty clamps the window to the screen: at the user's font size 140x36
  is the most that fits. `stills.sh` crops each shot to its slot's ratio.
- Downloading tools through `npx` may be refused; the HTML mails are
  written by hand in MJML's output style, which needs nothing.

## Content

Invented people and companies only, `.example` domains, no real brand.
A new mail goes in `demo/mailbox/`; an HTML one in `demo/mail/` with
`@DATE@`. Dates count back from the run, so the List always reads as
this week.
