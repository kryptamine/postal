# TUI UX patterns: lazygit, aerc, neomutt

Research for [#6](https://github.com/kryptamine/postal/issues/6), feeding
[#10](https://github.com/kryptamine/postal/issues/10). Question: what makes
lazygit's terminal UX work, what do aerc and neomutt teach for mail, and what
should Postal's two-pane MVP (Message List + Preview of the Inbox) borrow.

Goal stated by the user: lazygit's feel with vim keys, minimal chrome, `?` lists
the commands. Not a desktop client in a terminal.

Sources are primary: project docs, default config files and source code,
read in November 2026. Each claim links to the file that owns it.

## Sources

| Ref | Source |
|-----|--------|
| LG-KB | lazygit keybindings, [`docs/keybindings/Keybindings_en.md`](https://github.com/jesseduffield/lazygit/blob/master/docs/keybindings/Keybindings_en.md) |
| LG-CFG | lazygit config reference, [`docs/Config.md`](https://github.com/jesseduffield/lazygit/blob/master/docs/Config.md) |
| LG-DEF | lazygit defaults, [`pkg/config/user_config.go`](https://github.com/jesseduffield/lazygit/blob/master/pkg/config/user_config.go) |
| LG-HELP | lazygit `?` menu, [`pkg/gui/controllers/options_menu_action.go`](https://github.com/jesseduffield/lazygit/blob/master/pkg/gui/controllers/options_menu_action.go) |
| LG-BAR | lazygit bottom line, [`pkg/gui/options_map.go`](https://github.com/jesseduffield/lazygit/blob/master/pkg/gui/options_map.go) |
| LG-STATUS | lazygit status/toasts, [`pkg/gui/status/status_manager.go`](https://github.com/jesseduffield/lazygit/blob/master/pkg/gui/status/status_manager.go) |
| AE-BINDS | aerc default binds, [`config/binds.conf`](https://git.sr.ht/~rjarry/aerc/tree/master/item/config/binds.conf) |
| AE-BINDS5 | [`aerc-binds(5)`](https://man.archlinux.org/man/aerc-binds.5.en) |
| AE-CFG | [`aerc-config(5)`](https://man.archlinux.org/man/aerc-config.5.en) |
| NM-GUIDE | NeoMutt guide, [Getting started](https://neomutt.org/guide/gettingstarted.html) |
| NM-RC | [`neomuttrc(5)`](https://neomutt.org/man/neomuttrc) |
| GITUI | gitui [`KEY_CONFIG.md`](https://github.com/gitui-org/gitui/blob/master/KEY_CONFIG.md) |
| YAZI | yazi [`keymap-default.toml`](https://github.com/sxyazi/yazi/blob/main/yazi-config/preset/keymap-default.toml) |
| K9S | k9s [commands](https://k9scli.io/topics/commands/) |

Screenshots: lazygit's README GIFs
([repo](https://github.com/jesseduffield/lazygit#readme)), aerc
([aerc-mail.org](https://aerc-mail.org/)), NeoMutt
([neomutt.org](https://neomutt.org/)).

## Pattern catalogue

### 1. Panels and focus

- **lazygit**: side panels on the left (status, files, branches, commits,
  stash), one large *main* view on the right that always shows the detail of
  the selection in the focused side panel. `h`/`l` or arrows move between
  panels, `1`-`5` jump to one, and `showPanelJumps` prints the digit in the
  panel title (LG-DEF, LG-CFG). Focus is shown by border colour only:
  `activeBorderColor: [green, bold]` (LG-CFG). The main view is scrolled
  *without focusing it*: `J`/`K`, `<ctrl+d>`/`<ctrl+u>`, PgDn/PgUp (LG-KB).
  `<esc>` "Exit back to side panel" when the main view is focused (LG-KB).
- **aerc**: tabs, a folder sidebar (`sidebar-width`, default 22, `0` disables)
  and the message list; the viewer opens as a new tab on `<Enter>`. A list +
  preview split is opt-in via `:split`/`:vsplit` (`s`/`S`) or
  `message-list-split` (AE-BINDS, AE-CFG).
- **neomutt**: separate full-screen *menus*: index, pager, help, browser.
  Return opens the pager; a mini-index above the pager is opt-in with
  `pager_index_lines`, default `0` (NM-GUIDE, NM-RC).

Borrow: lazygit's "selection drives the detail view" rule: moving in the Message
List updates the Preview with no extra key. Scroll the Preview from the list
without moving focus. Border colour as the only focus cue.

Avoid: aerc/neomutt's open-a-separate-screen model as the default; tabs; a
folder sidebar (out of scope anyway); numbered panel jumps (two panes do not
need them).

### 2. Vim keymap conventions

Consensus across tools:

| Action | lazygit | aerc | yazi | neomutt |
|--------|---------|------|------|---------|
| next / prev item | `j`/`k`, arrows | `j`/`k`, arrows | `j`/`k` | `j`/`k` |
| top / bottom | `<`/`>`, Home/End | `g`/`G` | `gg`/`G` | — |
| half page | (main view) `<c-d>`/`<c-u>` | `<c-d>`/`<c-u>` | `<c-d>`/`<c-u>` | — |
| page | `,`/`.` | `<c-f>`/`<c-b>`, PgDn/PgUp | — | — |
| scroll detail view | `J`/`K` | (`J`/`K` = next/prev message in viewer) | `J`/`K` "seek preview" | — |
| search | `/` | `/`, `n`/`N` | `/`, `?` | `/` |
| cancel / back | `<esc>` | `<esc>` (clear), `q` closes viewer | `<esc>` | `q` |
| quit | `q`, `<c-c>` | `q` (with prompt) | `q` | `q` |
| help | `?` | `?` → `:help keys` | `~`, `F1` | `?` |
| suspend | `<c-z>` | `<c-z>` | — | — |

Sources: LG-KB, LG-DEF, AE-BINDS, YAZI, NM-GUIDE. Note lazygit's own `h`/`l`
panel keys are marked deprecated "alt" bindings in favour of arrows (LG-DEF),
and gitui ships arrows by default with an opt-in `vim_style_key_config.ron`
(GITUI). Postal can be vim-first since its user asked for it.

Borrow: `j`/`k`, `gg`/`G`, `<c-d>`/`<c-u>` for the list; `J`/`K` scroll the
Preview (lazygit and yazi agree); arrows as silent aliases; `<c-z>` suspend.

Avoid: aerc's two-key chords for common actions (`rr`, `Rq`, `pa`, `zc`) and
its ex command line `:` as the main way in (AE-BINDS5). Chords need a which-key
popup to be discoverable; Postal has too few commands to need them.

### 3. `?` help / keybinding menu

- **lazygit**: `?` opens a popup menu built from the live bindings of the
  current context, in three sections: *local* (this panel), *global*,
  *navigation*. It filters as you type, `<enter>` *runs* the selected
  binding, disabled bindings carry their reason, and keys align right in a
  column (LG-HELP). So the help menu doubles as a command palette.
- **neomutt**: "You can always type `?` in any menu to display the current
  bindings", as a full screen (NM-GUIDE).
- **aerc**: `?` runs `:help keys` (AE-BINDS). yazi's help (`~`/`F1`) lets
  `<Enter>` "close and run the selected action" (YAZI). k9s: `?` "Show active
  keyboard mnemonics and help" (K9S).

Borrow: lazygit's model exactly: a popup over the UI, generated from the same
binding table that dispatches keys (never a hand-kept list), context first,
filter as you type, Enter runs. `<esc>` or `?` closes.

### 4. Status line and key hints

- **lazygit**: `showBottomLine: true` shows "keybinding info and useful
  buttons" (LG-CFG). Only bindings flagged `DisplayOnScreen` and not disabled
  appear, formatted `description: key`, joined by ` | `, and cut with `…` at
  the terminal width (LG-BAR). `showListFooter` prints "5 of 20" under lists
  (LG-CFG).
- **neomutt**: a help line on the first screen row (`help`, default yes) plus a
  status line near the bottom (NM-RC, NM-GUIDE).

Borrow: one bottom line, a hand-picked handful of hints for the focused pane,
truncated with `…`, always ending in `?: help`. A position counter
("12 of 340") in the list's border.

Avoid: neomutt's second chrome line at the top.

### 5. Popups and modals

lazygit uses one popup type for menus, confirmations and prompts: `<enter>`
confirms or runs, `<esc>` closes, `/` filters (LG-KB "Menu", "Confirmation
panel"). `confirmOnQuit` defaults to `false` (LG-DEF). aerc prompts on quit
(`:prompt 'Quit?' quit`) (AE-BINDS).

Borrow: one popup widget, Enter/Esc semantics. No quit confirmation: Postal is
read-only, quitting loses nothing.

### 6. Responsive behaviour on narrow terminals

- **lazygit**: `portraitMode: auto` stacks panels vertically when width ≤
  `portraitModeAutoMaxWidth` (84) and height ≥ `portraitModeAutoMinHeight`
  (46). `mainPanelSplitMode: flexible` splits horizontally "if the window is
  wide enough, otherwise split vertically". `sidePanelWidth` defaults to
  0.3333. `+`/`_` cycle screen modes normal/half/full for the focused window
  (LG-CFG, LG-KB).
- **aerc**: `index-columns` default `flags:4,name<20%,subject,date>=`, widths
  in cells or percentages; relative date formats (`this-day-time-format` etc.)
  keep the date column short (AE-CFG).

Borrow: side-by-side on wide terminals, stacked list-over-preview when narrow,
with lazygit's thresholds as a starting point. A zoom key (`+`/`_`) cycling the
focused pane through normal/half/full. Fixed-width flags and date columns with
the sender as a percentage and the subject taking the rest; relative dates
("14:02", "Tue", "Mar 3").

### 7. Announcing new items and background work

- **lazygit**: background work shows a *waiting status* with a spinner; a toast
  disappears after 2 s, an error toast after 4 s (LG-STATUS).
- **aerc**: `new-message-bell` rings the bell on new mail, default `true`;
  `spinner` while loading (AE-CFG).
- **neomutt**: `beep_new` default *no*; `new_mail_command` runs a hook;
  `mark_old` turns unread into old on leaving a mailbox (NM-RC).

Borrow: lazygit's transient status in the bottom line ("syncing…" with spinner,
"2 new" toast for 2 s). New Messages appear at the top of the list with an
unread marker; the cursor stays on the Message the user was reading.

Avoid: the bell by default; neomutt's new/old/unread distinction (Postal never
writes flags).

### 8. Rendering HTML

aerc pipes parts through `[filters]` by MIME type, e.g.
`text/html=pandoc -f html -t plain` or `w3m`; first match wins (AE-CFG).
Postal already decided to render HTML through its own converter with numbered
link footnotes (#1), which avoids an external tool dependency.

## What to avoid overall

- Desktop-client chrome: folder sidebars, tabs, menu bars, a top help line.
- An ex command line (`:`) as a primary interface; customisation-first configs
  (aerc's per-context binds files) before the defaults are right.
- Multi-key chords for frequent actions.
- Confirmations for reversible or harmless actions.

## Recommendation

1. **Layout**: two panes. Message List on the left (about a third of the width)
   and Preview on the right; stacked list-over-preview below about 84 columns.
   One bottom line. No sidebar, tabs or title bar.
2. **Focus**: the Message List always has focus in normal use, and its
   selection drives the Preview. The Preview scrolls from the list without a
   focus change: `J`/`K` by line, `<space>`/`<s-space>` by page. `<enter>` or `l` focuses
   the Preview (then `j`/`k` scroll it), `<esc>` or `h` returns. Focus shown by
   border colour only.
3. **Keys**: `j`/`k`, `gg`/`G`, `<c-d>`/`<c-u>`, `+`/`_`
   zoom, `q` quits without asking, `<c-z>` suspends, `?` help. Arrows,
   Home/End and PgUp/PgDn as aliases. No chords besides `gg`.
4. **`?`**: a lazygit-style popup generated from the binding table, sections
   *Message List* / *Preview* / *Global*, filter as you type, `<enter>` runs.
5. **Bottom line**: the 3-5 most useful keys for the focused pane, `…` when cut,
   always `?: help`; right side holds sync status (spinner while syncing,
   transient "N new" for 2 s, errors for 4 s).
6. **New mail**: inserted at the top with an unread marker, selection held on
   the current Message, no bell.

Open questions for #10: whether `<enter>` focuses the Preview or zooms it full
screen; the exact narrow-terminal breakpoint; whether `/` searches or filters
(search is out of MVP scope per #1, so `/` may simply be absent at first).
