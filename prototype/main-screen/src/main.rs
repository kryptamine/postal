// PROTOTYPE, throwaway. Three main-screen layouts for Postal on fake Messages.
// Prototype keys (top bar): 1/2/3 variant, e Enter mode, [ ] cutoff, s show `/`.
use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    DefaultTerminal, Frame,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, PartialEq)]
enum Variant {
    SideBySide,
    FullScreen,
    Adaptive,
}

impl Variant {
    fn label(self) -> &'static str {
        match self {
            Self::SideBySide => "A side-by-side",
            Self::FullScreen => "B full-screen list / full-screen Preview",
            Self::Adaptive => "C adaptive (side-by-side, stacked when narrow)",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    List,
    Preview,
}

#[derive(Clone, Copy, PartialEq)]
enum EnterMode {
    FocusPreview,
    ZoomPreview,
}

struct Msg {
    unread: bool,
    sender: &'static str,
    subject: &'static str,
    date: &'static str,
    snippet: &'static str,
    body: String,
}

struct App {
    variant: Variant,
    enter_mode: EnterMode,
    cutoff: u16,
    show_slash: bool,
    three_line: bool,
    msgs: Vec<Msg>,
    selected: usize,
    list_offset: usize,
    focus: Focus,
    zoomed: bool,
    scroll: u16,
    help: bool,
    toast: Option<String>,
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = App {
        variant: Variant::Adaptive,
        enter_mode: EnterMode::FocusPreview,
        cutoff: 84,
        show_slash: false,
        three_line: true,
        msgs: fake_messages(),
        selected: 0,
        list_offset: 0,
        focus: Focus::List,
        zoomed: false,
        scroll: 0,
        help: false,
        toast: None,
    };
    loop {
        terminal.draw(|f| draw(f, &mut app))?;
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        app.toast = None;
        if app.help {
            app.help = false;
            continue;
        }
        match key.code {
            KeyCode::Char('1') => set_variant(&mut app, Variant::SideBySide),
            KeyCode::Char('2') => set_variant(&mut app, Variant::FullScreen),
            KeyCode::Char('3') => set_variant(&mut app, Variant::Adaptive),
            KeyCode::Char('e') => {
                app.enter_mode = match app.enter_mode {
                    EnterMode::FocusPreview => EnterMode::ZoomPreview,
                    EnterMode::ZoomPreview => EnterMode::FocusPreview,
                };
            }
            KeyCode::Char('[') => app.cutoff = app.cutoff.saturating_sub(4).max(40),
            KeyCode::Char(']') => app.cutoff = (app.cutoff + 4).min(200),
            KeyCode::Char('s') => app.show_slash = !app.show_slash,
            KeyCode::Char('r') => app.three_line = !app.three_line,
            KeyCode::Char('?') => app.help = true,
            KeyCode::Char('/') => app.toast = Some("search is not in the MVP".into()),
            KeyCode::Char('j') | KeyCode::Down if app.focus == Focus::List && !app.zoomed => {
                move_sel(&mut app, 1);
            }
            KeyCode::Char('k') | KeyCode::Up if app.focus == Focus::List && !app.zoomed => {
                move_sel(&mut app, -1);
            }
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Char('J') => {
                app.scroll = app.scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::Char('K') => {
                app.scroll = app.scroll.saturating_sub(1);
            }
            KeyCode::Char('g') => select(&mut app, 0),
            KeyCode::Char('G') => {
                let last = app.msgs.len() - 1;
                select(&mut app, last);
            }
            KeyCode::Enter | KeyCode::Char('l') => open(&mut app),
            KeyCode::Tab if !preview_hidden(&app, 0) => {
                app.focus = if app.focus == Focus::List { Focus::Preview } else { Focus::List };
            }
            KeyCode::Esc | KeyCode::Char('h') => back(&mut app),
            KeyCode::Char('q') => {
                if app.zoomed || app.focus == Focus::Preview {
                    back(&mut app);
                } else {
                    return Ok(());
                }
            }
            _ => {}
        }
    }
}

fn set_variant(app: &mut App, v: Variant) {
    app.variant = v;
    app.zoomed = false;
    app.focus = Focus::List;
}

fn select(app: &mut App, i: usize) {
    if i != app.selected {
        app.selected = i;
        app.scroll = 0;
    }
}

fn move_sel(app: &mut App, d: isize) {
    let i = (app.selected as isize + d).clamp(0, app.msgs.len() as isize - 1) as usize;
    select(app, i);
}

fn open(app: &mut App) {
    if app.variant == Variant::FullScreen || app.enter_mode == EnterMode::ZoomPreview {
        app.zoomed = true;
        app.focus = Focus::Preview;
    } else {
        app.focus = Focus::Preview;
    }
}

fn back(app: &mut App) {
    app.zoomed = false;
    app.focus = Focus::List;
}

// True when the current layout shows the list alone (variant B, not zoomed).
fn preview_hidden(app: &App, _w: u16) -> bool {
    app.variant == Variant::FullScreen && !app.zoomed
}

fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let [bar, main, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_proto_bar(f, bar, app, area.width);
    draw_hints(f, hints, app);

    if main.width < 20 || main.height < 5 {
        f.render_widget(Paragraph::new("terminal too small"), main);
        return;
    }

    if app.zoomed {
        draw_preview(f, main, app, true);
    } else {
        match app.variant {
            Variant::FullScreen => draw_list(f, main, app, true),
            Variant::SideBySide => side_by_side(f, main, app),
            Variant::Adaptive => {
                if area.width >= app.cutoff {
                    side_by_side(f, main, app);
                } else {
                    let [l, p] = Layout::vertical([
                        Constraint::Percentage(45),
                        Constraint::Percentage(55),
                    ])
                    .areas(main);
                    draw_list(f, l, app, false);
                    draw_preview(f, p, app, false);
                }
            }
        }
    }
    if app.help {
        draw_help(f, main);
    }
}

fn side_by_side(f: &mut Frame, main: Rect, app: &mut App) {
    let [l, p] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(main);
    draw_list(f, l, app, false);
    draw_preview(f, p, app, false);
}

fn draw_proto_bar(f: &mut Frame, area: Rect, app: &App, width: u16) {
    let layout = if app.zoomed {
        "zoomed"
    } else {
        match app.variant {
            Variant::Adaptive if width >= app.cutoff => "side-by-side",
            Variant::Adaptive => "stacked",
            _ => "-",
        }
    };
    let enter = match app.enter_mode {
        EnterMode::FocusPreview => "focus",
        EnterMode::ZoomPreview => "zoom",
    };
    let text = format!(
        " PROTOTYPE  {}  | rows {} | width {width} cutoff {} -> {layout} | Enter={enter} | / {} |  1/2/3 e [ ] s r ",
        app.variant.label(),
        if app.three_line { "3-line" } else { "1-line" },
        app.cutoff,
        if app.show_slash { "shown" } else { "hidden" },
    );
    f.render_widget(
        Paragraph::new(text).style(Style::default().bg(Color::Magenta).fg(Color::Black)),
        area,
    );
}

fn border(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn draw_list(f: &mut Frame, area: Rect, app: &mut App, full: bool) {
    let focused = full || app.focus == Focus::List;
    let unread = app.msgs.iter().filter(|m| m.unread).count();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border(focused))
        .title(format!(" Inbox  {unread} unread "));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let per = if app.three_line { 3 } else { 1 };
    let h = inner.height as usize / per;
    if app.selected < app.list_offset {
        app.list_offset = app.selected;
    } else if app.selected >= app.list_offset + h {
        app.list_offset = app.selected + 1 - h;
    }
    let w = inner.width as usize;
    let lines: Vec<Line> = app
        .msgs
        .iter()
        .enumerate()
        .skip(app.list_offset)
        .take(h)
        .flat_map(|(i, m)| {
            if app.three_line {
                card(m, w, i == app.selected, focused)
            } else {
                vec![row(m, w, i == app.selected, focused)]
            }
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}

fn row(m: &Msg, w: usize, selected: bool, focused: bool) -> Line<'static> {
    let sender_w = (w / 4).clamp(12, 24);
    let show_date = w >= 40;
    let date_w = if show_date { 10 } else { 0 };
    let subject_w = w.saturating_sub(2 + sender_w + 2 + 2 + date_w);
    let mut text = String::new();
    text.push_str(if m.unread { "● " } else { "  " });
    text.push_str(&fit(m.sender, sender_w));
    text.push_str("  ");
    text.push_str(&fit(m.subject, subject_w));
    if show_date {
        text.push_str("  ");
        text.push_str(&format!("{:>10}", m.date));
    }
    let mut style = Style::default();
    if m.unread {
        style = style.add_modifier(Modifier::BOLD);
    }
    if selected {
        style = if focused {
            style.bg(Color::Blue).fg(Color::White)
        } else {
            style.bg(Color::DarkGray)
        };
    }
    Line::from(Span::styled(text, style))
}

fn card(m: &Msg, w: usize, selected: bool, focused: bool) -> Vec<Line<'static>> {
    let date_w = m.date.width();
    let sender_w = w.saturating_sub(2 + 2 + date_w);
    let mut base = Style::default();
    if selected {
        base = if focused { base.bg(Color::Blue).fg(Color::White) } else { base.bg(Color::DarkGray) };
    }
    let mut head = base;
    if m.unread {
        head = head.add_modifier(Modifier::BOLD);
    }
    let dim = if selected { base } else { base.fg(Color::DarkGray) };
    let mark = if m.unread { "● " } else { "  " };
    vec![
        Line::from(vec![
            Span::styled(format!("{mark}{}  ", fit(m.sender, sender_w)), head),
            Span::styled(m.date.to_owned(), dim),
        ]),
        Line::from(Span::styled(format!("  {}", fit(m.subject, w.saturating_sub(2))), head)),
        Line::from(Span::styled(format!("  {}", fit(m.snippet, w.saturating_sub(2))), dim)),
    ]
}

// Pad or cut to exactly `w` display columns, ending in `…` when cut.
fn fit(s: &str, w: usize) -> String {
    if s.width() <= w {
        return format!("{s}{}", " ".repeat(w - s.width()));
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if used + cw + 1 > w {
            break;
        }
        out.push(c);
        used += cw;
    }
    out.push('…');
    used += 1;
    out.push_str(&" ".repeat(w.saturating_sub(used)));
    out
}

fn draw_preview(f: &mut Frame, area: Rect, app: &App, zoomed: bool) {
    let focused = zoomed || app.focus == Focus::Preview;
    let m = &app.msgs[app.selected];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border(focused))
        .title(format!(" {}/{} ", app.selected + 1, app.msgs.len()));
    let mut lines = vec![
        Line::from(vec![Span::styled("From    ", Style::default().fg(Color::DarkGray)), Span::raw(m.sender)]),
        Line::from(vec![
            Span::styled("Subject ", Style::default().fg(Color::DarkGray)),
            Span::styled(m.subject, Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![Span::styled("Date    ", Style::default().fg(Color::DarkGray)), Span::raw(m.date)]),
        Line::raw(""),
    ];
    lines.extend(m.body.lines().map(|l| Line::raw(l.to_owned())));
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((app.scroll, 0)),
        area,
    );
}

fn draw_hints(f: &mut Frame, area: Rect, app: &App) {
    let left = if let Some(t) = &app.toast {
        t.clone()
    } else {
        let mut keys: Vec<&str> = if app.zoomed {
            vec!["j/k: scroll", "q/h: back"]
        } else if app.focus == Focus::Preview {
            vec!["j/k: scroll", "h/esc: list", "tab: list"]
        } else if app.variant == Variant::FullScreen {
            vec!["j/k: move", "enter: open", "q: quit"]
        } else {
            vec!["j/k: move", "J/K: scroll", "enter: read", "q: quit"]
        };
        if app.show_slash {
            keys.push("/: search");
        }
        keys.push("?: help");
        keys.join("  ")
    };
    let right = "⟳ synced 14:05 ";
    let w = area.width as usize;
    let left_w = w.saturating_sub(right.width() + 1);
    let line = Line::from(vec![
        Span::styled(fit(&left, left_w), Style::default().fg(Color::Cyan)),
        Span::raw(" "),
        Span::styled(right, Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_help(f: &mut Frame, main: Rect) {
    let w = 46.min(main.width);
    let h = 16.min(main.height);
    let area = Rect::new(main.x + (main.width - w) / 2, main.y + (main.height - h) / 2, w, h);
    let text = vec![
        Line::styled("Message List", Style::default().add_modifier(Modifier::BOLD)),
        Line::raw("  j/k ↓/↑   move selection"),
        Line::raw("  g/G       first / last"),
        Line::raw("  J/K       scroll Preview"),
        Line::raw("  enter/l   read (focus or zoom Preview)"),
        Line::raw("  tab       switch pane"),
        Line::raw(""),
        Line::styled("Preview", Style::default().add_modifier(Modifier::BOLD)),
        Line::raw("  j/k       scroll"),
        Line::raw("  h/esc/q   back to list"),
        Line::raw(""),
        Line::styled("Global", Style::default().add_modifier(Modifier::BOLD)),
        Line::raw("  ?         this help"),
        Line::raw("  q         quit (from list)"),
    ];
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Keys ")),
        area,
    );
}

fn fake_messages() -> Vec<Msg> {
    let rows: &[(bool, &str, &str, &str)] = &[
        (true, "GitHub", "[kryptamine/postal] Prototype the main screen layout (#10)", "14:05"),
        (true, "Anna Kowalska", "Re: dinner on Friday?", "13:41"),
        (false, "Stripe", "Your receipt from Fly.io #2931-0042", "11:02"),
        (true, "Hacker News Digest", "Top stories: Rust 1.94 released, SQLite internals, and a very long subject that will not fit", "09:15"),
        (false, "no-reply@accounts.google.com", "Security alert: new sign-in on Mac", "08:30"),
        (false, "Linear", "Weekly summary for Postal", "16 Nov"),
        (false, "田中 太郎", "会議の議事録 — 第3四半期レビュー", "16 Nov"),
        (true, "Mum", "🎂 don't forget grandma's birthday", "15 Nov"),
        (false, "AWS Notifications", "AWS Notification - Subscription Confirmation", "14 Nov"),
        (false, "Jonathan Alexander Featherstonehaugh-Smythe", "Long sender name test", "12 Nov"),
        (false, "Booking.com", "Your booking in Lisbon is confirmed", "10 Nov"),
        (false, "Figma", "(no subject)", "08 Nov"),
        (false, "crates.io", "Your API token is about to expire", "02 Nov"),
        (false, "Spotify", "Your 2026 Wrapped is almost here", "28 Oct"),
        (false, "Rust Weekly", "This Week in Rust 620", "22 Oct"),
        (false, "Dmitry Petrov", "Re: Re: Re: postal ideas", "2025-12-30"),
        (false, "Apple", "Your receipt from Apple.", "2025-11-04"),
        (false, "Old Friend", "long time no see", "2024-11-17"),
    ];
    let snippets: &[&str] = &[
        "kryptamine assigned this to you. Which layout does the main screen use: side by side, full-screen or adaptive?",
        "Sure, Friday works! Shall we try that new Georgian place near the station? I can book for 8.",
        "Receipt #2931-0042. Amount paid $12.40. Fly.io usage for October 2026. Questions? Contact support.",
        "1. Rust 1.94 released (412 points) 2. How SQLite stores a B-tree page 3. Show HN: a terminal mail client",
        "New sign-in to your Google Account on a Mac device. If this was you, you don't need to do anything.",
        "3 issues completed, 2 in progress. Cycle 14 ends in 4 days. View your team's progress in Linear.",
        "お疲れ様です。第3四半期レビューの議事録を共有します。ご確認ください。",
        "Her birthday is on Sunday!! Call her in the morning, she's going to the theatre in the evening 💐",
        "You have chosen to subscribe to the topic arn:aws:sns:eu-central-1:1234:alerts. To confirm, visit",
        "Hello, I'm writing to check how Postal renders a sender name that is far longer than any column.",
        "Hotel Avenida, Lisbon. Check-in Thu 4 Dec from 15:00. Confirmation number 4123.881.209.",
        "",
        "Your crates.io API token 'laptop' expires in 7 days. Generate a new one at crates.io/settings/tokens.",
        "Get ready: your top artists, songs and genres of the year land on 4 December.",
        "Hello and welcome to another issue of This Week in Rust! Updates from the Rust community.",
        "> > > What if it only shows the inbox? I'd actually use that every day. Let's keep it tiny.",
        "Apple ID receipt. iCloud+ with 200 GB storage, monthly. Total 2,99 €.",
        "Hey! Found your address in an old notebook. How have you been? Still writing code?",
    ];
    let mut out = Vec::new();
    for round in 0..3 {
        for (n, (unread, sender, subject, date)) in rows.iter().enumerate() {
            out.push(Msg {
                unread: *unread && round == 0,
                sender,
                subject,
                date,
                snippet: snippets[n],
                body: fake_body(sender),
            });
        }
    }
    out
}

fn fake_body(sender: &str) -> String {
    format!(
        "Hi Alex,\n\n\
         This is a fake Message from {sender} so the Preview has something to show. \
         A typical HTML newsletter, after conversion, turns into a few paragraphs of plain \
         text with links pulled out as numbered footnotes, like this one [1] and that one [2].\n\n\
         Second paragraph: lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do \
         eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, \
         quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.\n\n\
         • a bullet point\n\
         • another bullet with a longer line that has to wrap when the Preview pane is narrow\n\n\
         > quoted reply text from an earlier message\n\
         > second quoted line\n\n\
         Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu \
         fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident.\n\n\
         Thanks,\n{sender}\n\n\
         [1] https://example.com/a/very/long/tracking/link?utm_source=newsletter&utm_medium=email\n\
         [2] https://example.org/unsubscribe\n"
    )
}
