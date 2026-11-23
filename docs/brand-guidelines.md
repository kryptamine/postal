# Brand Guidelines v0.2

> Last updated: 2026-11-23
> Status: Draft

One source for how Postal looks and speaks: the landing page, the README,
the GitHub social preview, release notes, the Neovim plugin's docs. The TUI
is the exception: it draws in the terminal's own palette (see **Scope**).

## Quick Reference

| Element | Value |
|---------|-------|
| Primary Color | #B5361F |
| Secondary Color | #2B4C9B |
| Accent Color | #C99A2E |
| Heading Font | Schibsted Grotesk |
| Mono Font | JetBrains Mono |
| Voice | Dry, precise, factual |
| Idea | The post office, at terminal speed |

---

## 1. Essence

**What Postal is:** a mail client for the terminal, written in Rust. Gmail,
iCloud and any IMAP server. It lays HTML mail out the way a mail client
does, in terminal cells, and keeps everything on the user's machine.

**The idea:** the post office, at terminal speed. Postal borrows the
visual language of real mail (postmarks, stamps, airmail stripes, paper)
and sets it on the grid of a terminal. The post office stands for trust and
care with someone's letters; the terminal stands for speed and control.

**Who it is for:** people who live in a terminal and in Neovim, and want
their mail there without giving up HTML mail or speed.

**What sets it apart** (every claim here is backed by the code or an ADR):

| Claim | Backing |
|-------|---------|
| HTML mail laid out, not stripped to text | litehtml on a cell grid, ADR 0006 |
| Instant | ≤100 ms cold start, ≤16 ms per frame, ≤30 MB RSS, ADR 0001 |
| Local | SQLite Cache, secrets in the OS keychain, no telemetry, no servers of its own |
| Live | the Inbox kept live through IDLE; works offline from the Cache |
| Made for Neovim | compose in `$EDITOR`; plugin with recipient completion and an agent beside a Draft |

---

## 2. Voice & Tone

### Brand Personality

| Trait | We Are | We Are Not |
|-------|--------|------------|
| Precise | Numbers, names of things, exact keys | Vague ("blazing fast", "lots of features") |
| Dry | Plain statements, one idea a sentence | Jokey, exclamation marks, emoji |
| Respectful | Of the reader's time and of their mail | Salesy, urgent, pushy |
| Confident | States what it does and stops | Comparing itself down to other clients |

### Rules

- Say a fact instead of an adjective: "Opens in 100 ms", not "lightning fast".
- One idea per sentence. Short sentences. Active voice.
- Name things as `GLOSSARY.md` does: Message, Thread, Folder, Account,
  Draft, Cache, Provider. Capitalised the same way.
- Keys and commands in code: `postal login`, `/`, `o`.
- No promise the code does not keep. A claim on the site links to its proof
  when it can (an ADR, the source).

### Tone by Context

| Context | Tone | Example |
|---------|------|---------|
| Landing headline | Short, factual | "Mail at terminal speed." |
| Feature copy | What it does, then the detail | "Search every cached Folder. Results narrow as you type." |
| Install docs | Instructional, imperative | "Run `postal`. Pick a Provider." |
| Privacy | Plain, complete, no legalese where avoidable | "Your mail stays on your machine." |
| Release notes | What changed, for whom | "Forward a Message with its attachments." |

### Prohibited Terms

| Avoid | Use instead |
|-------|-------------|
| Blazing / lightning fast | the measured number |
| Revolutionary, game-changing | say what changed |
| Seamless, effortless | say what the user does |
| Powerful | name the capability |
| Email (as a noun for one item) | Message |
| User-friendly, intuitive | show the screenshot |

---

## 3. Messaging

### Taglines (candidates)

1. **Mail at terminal speed.** (primary)
2. Your mail, in your terminal, on your machine.
3. HTML mail, laid out in cells.

### Key Messages

| Message | Proof point |
|---------|-------------|
| It reads real mail | A newsletter in a browser beside the same mail in Postal |
| It is instant | Cold start, frame time and memory budgets |
| It is yours | Local Cache and keychain; nothing sent anywhere but your mail servers |
| It fits your editor | Compose in `$EDITOR`; the Neovim plugin and its agent pane |

---

## 4. Color Palette

The palette is paper and ink with the two colours of an airmail envelope.
Light is the default theme; dark is a full theme, not an inversion.

### Primary Colors

| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| Postbox Red | #B5361F | rgb(181,54,31) | Logo, primary buttons, links on paper |
| Postbox Red Night | #F0674A | rgb(240,103,74) | The same roles on graphite |

### Secondary Colors

| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| Airmail Blue | #2B4C9B | rgb(43,76,155) | Airmail stripe, secondary links, focus rings |
| Airmail Blue Night | #8FA9F0 | rgb(143,169,240) | The same roles on graphite |

### Accent Colors

| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| Stamp Ochre | #C99A2E | rgb(201,154,46) | Fills, stamp and postmark details; never text on paper |
| Stamp Ochre Night | #E0B04F | rgb(224,176,79) | The same, on graphite; may be text |

### Neutral Palette

| Name | Light | Dark | Usage |
|------|-------|------|-------|
| Background | #F4EFE6 (paper) | #171614 (graphite) | Page |
| Surface | #EBE4D7 | #22201D | Cards, code blocks, media frames |
| Text | #1C1B19 (ink) | #ECE6DA | Headings, body |
| Text Muted | #5E5A53 | #A39D92 | Captions, secondary copy |
| Rule | #D6CCBB | #3A3733 | Borders, dividers (decorative, not for text) |

### Semantic Colors

Red is the brand, so an error never relies on colour alone: it carries an
icon and words.

| State | Light | Dark |
|-------|-------|------|
| Success | #2F6B45 | #7CC495 |
| Warning | #8A6414 | #E0B04F |
| Error | #B5361F | #F0674A |
| Info | #2B4C9B | #8FA9F0 |

### Accessibility (WCAG 2.1, measured)

| Pair | Ratio |
|------|-------|
| Ink on paper | 15.0:1 |
| Muted on paper | 6.0:1 |
| Postbox Red on paper | 5.2:1 |
| Postbox Red on surface | 4.7:1 |
| Airmail Blue on paper | 7.0:1 |
| Paper on Postbox Red (button label) | 5.2:1 |
| Text on graphite | 14.6:1 |
| Muted on graphite | 6.7:1 |
| Postbox Red Night on graphite | 5.8:1 |
| Graphite on Postbox Red Night (button label) | 5.8:1 |

Stamp Ochre is 4.2:1 on surface: decoration only in light.

---

## 5. Typography

The mono face matches the terminal in the screenshots, so the page and the
app read as one thing.

```css
--font-heading: 'Schibsted Grotesk', system-ui, -apple-system, sans-serif;
--font-body: 'Schibsted Grotesk', system-ui, -apple-system, sans-serif;
--font-mono: 'JetBrains Mono', ui-monospace, 'SF Mono', Menlo, monospace;
```

| Role | Font | Where |
|------|------|-------|
| Headings, display | Schibsted Grotesk 700/800 | Titles, the hero line |
| Long body | Schibsted Grotesk 400 | Privacy policy, terms, long sections |
| Landing body, labels, UI | JetBrains Mono 400/500 | Feature copy, buttons, captions, nav |
| Code, keys, commands | JetBrains Mono 400 | `postal login`, key caps |

### Type Scale

| Element | Desktop | Mobile | Weight | Line Height |
|---------|---------|--------|--------|-------------|
| Display | 64px | 40px | 800 | 1.05 |
| H1 | 48px | 32px | 700 | 1.1 |
| H2 | 32px | 26px | 700 | 1.2 |
| H3 | 22px | 20px | 600 | 1.3 |
| Body (grotesk) | 18px | 17px | 400 | 1.6 |
| Body (mono) | 15px | 15px | 400 | 1.6 |
| Small / caption | 13px | 13px | 400 | 1.5 |

Headings set tight (letter-spacing −0.02em); mono never gets negative
tracking. Widths: body text at most 68 characters a line.

Both families are SIL Open Font License, served from Google Fonts or
self-hosted.

---

## 6. Motifs

Used sparingly: one motif per section at most.

| Motif | Form | Where |
|-------|------|-------|
| Airmail stripe | Diagonal red and blue bars, 45° | A thin edge on the hero or the footer, never a full background |
| Postmark | Circle with the text on its rim and wavy cancellation lines | Badges for measured facts ("≤100 ms") |
| Stamp | Rectangle with a perforated edge | The frame of a screenshot or video |
| Cell grid | The terminal's character grid | Layout: spacing in multiples of a cell |

---

## 7. Logo

**The stamp prompt:** a perforated postage stamp with the shell prompt
`›_` cut out of it. Post and terminal in one solid shape.

- **Grid:** 48 units. The stamp is 34 × 34 at (7, 7), with four
  perforations of radius 2.2 on each edge; the chevron is a 3.6-unit
  round stroke, the cursor a bar 8.5 × 3.4.
- **Small cut** (24 px and below): the perforations would turn to noise,
  so it keeps one notch per edge, with a heavier prompt.
- **Wordmark:** `postal` in Schibsted Grotesk 800, lowercase, tracked
  −0.03em. Its baseline sits on the cursor's bottom edge; the gap to the
  stamp is 8 units.
- **Files are pure paths:** no fonts, no masks, so every renderer draws
  them the same.

### Files

All in `assets/brand/`, drawn by `scripts/brand.py` (`make brand`):
edit the script, not the files. Each colour comes as its own file:
Postbox Red (no suffix), `-night` for graphite, `-ink` and `-paper` for
one colour.

| Variant | File | Use Case |
|---------|------|----------|
| Lockup | postal-logo*.svg, postal-logo-1024.png | Site header, README |
| Mark | postal-mark*.svg | Avatar, anywhere above 24 px |
| Mark, small cut | postal-mark-small*.svg | 24 px and below |
| Wordmark | postal-wordmark.svg | Where the mark is already shown |
| Favicon | favicon.svg (follows the tab's theme), favicon-16/32.png | Browser tabs |
| App icon | postal-app-icon.svg, -512.png, apple-touch-icon.png | Graphite tile, night-red stamp |
| Social preview | social-preview.svg/.png (1280 × 640) | GitHub, link cards |
| Terminal | postal-mark.txt | Onboarding, `--version` |

Clear space round the mark is one perforation pitch (8 units).

### Don'ts

- Don't recolour outside the palette, add shadows, gradients or outlines.
- Don't rotate, skew or crop.
- Don't set the mark on a screenshot or a busy background.

---

## 8. Imagery

- **Screenshots and videos only of the demo mailbox** (issue #115): no
  real person's mail, no real company's brand or logo.
- One terminal, one font (JetBrains Mono Nerd Font), fixed window size,
  shot at 2×. A dark and a light version of each, matching the site theme.
- Framed by the stamp motif or a plain surface card; no fake window chrome
  with traffic lights.
- No stock photos, no illustrations of people, no 3D renders, no AI images.
- Videos: MP4/WebM, muted, looping, 5–10 s, one action each.

---

## 9. Components

### Buttons

| Type | Background | Text | Border |
|------|------------|------|--------|
| Primary | Postbox Red | Paper | none |
| Secondary | transparent | Ink | 1px Rule, Ink on hover |
| Code (install) | Surface | Ink, mono | 1px Rule |

Buttons are mono, 500, no uppercase.

### Spacing

A cell is 8px. Spacing is 4, 8, 16, 24, 32, 48, 64, 96px.

### Radius

Small, like a terminal: 4px for buttons and inputs, 6px for cards and
media frames, 9999px only for key caps and pills.

---

## 10. Scope

- **The TUI does not take this palette.** Postal colours its screen from
  the terminal's palette, so it fits any theme the user runs. The brand
  lives around the app, not inside it; the logo's terminal variant is the
  one bridge.
- These guidelines are the source; design tokens (CSS variables, JSON) are
  generated from them and kept beside the logo files in `assets/brand/`.

---

## Changelog

| Version | Date | Changes |
|---------|------|---------|
| 0.1 | 2026-11-23 | First draft: essence, voice, palette, type, motifs, logo brief |
| 0.2 | 2026-11-23 | Logo: the stamp prompt, its files and exports |
