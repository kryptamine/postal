//! Postal on one HTML Message, and questions about the Preview pane it
//! draws at 120 columns.

use std::time::{Duration, Instant};

use termlens::{Color, Key, Screen, Terminal};

use super::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};

pub const WIDE: (u16, u16) = (120, 40);

pub const DARK: (u8, u8, u8) = (0, 0, 0);

pub const LIGHT: (u8, u8, u8) = (255, 255, 255);

pub struct Run {
    pub size: (u16, u16),
    pub background: (u8, u8, u8),
    pub no_color: bool,
}

pub const DEFAULT: Run = Run {
    size: WIDE,
    background: DARK,
    no_color: false,
};

pub fn fixture(name: &str) -> FakeBody {
    let path = format!("{}/tests/fixtures/{name}.eml", env!("CARGO_MANIFEST_DIR"));
    FakeBody::eml(&std::fs::read(path).unwrap())
}

pub fn html(content: &str) -> FakeBody {
    FakeBody::text("html", "7bit", content)
}

/// Postal on the one Message `body`, once sync is done.
pub fn started(body: FakeBody, run: &Run) -> (Terminal, FakeImap) {
    let message = FakeMessage::new("Sender", "Subject", "2024-11-18T09:00:00Z").body(body);
    let server = FakeImap::start(vec![message]);
    let home = Home::for_server(&server);
    let (r, g, b) = run.background;
    let builder = Terminal::builder()
        .size(run.size.0, run.size.1)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .background_rgb(r, g, b)
        .timeout(Duration::from_secs(10));
    let builder = if run.no_color {
        builder.env("NO_COLOR", "1")
    } else {
        builder
    };

    let t = builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !server.commands().iter().any(|c| c == "IDLE") {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }

    (t, server)
}

pub fn quit(mut t: Terminal) {
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// The screen once the Preview of `body` shows `shown`.
pub fn preview(body: FakeBody, run: &Run, shown: &str) -> Screen {
    let (mut t, _server) = started(body, run);
    let screen = t.snapshot_after(|s| s.contains(shown)).unwrap();
    quit(t);
    screen
}

/// The Preview's rows right of the list at 120 columns, trailing blanks cut.
pub fn pane(screen: &Screen) -> Vec<String> {
    screen
        .rect_text(44..118, 4..39)
        .lines()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

/// The screen row and column where `text` starts, in the Preview pane.
pub fn at(screen: &Screen, text: &str) -> (u16, u16) {
    (4..39)
        .find_map(|row| {
            let line: Vec<char> = screen.row_text(row).chars().collect();
            let needle: Vec<char> = text.chars().collect();
            let col = (44..line.len()).find(|&c| line[c..].starts_with(&needle))?;
            Some((row, u16::try_from(col).unwrap()))
        })
        .unwrap_or_else(|| panic!("no {text:?} in\n{}", screen.text()))
}

pub fn bg(screen: &Screen, row: u16, col: u16) -> Color {
    screen.cell(row, col).unwrap().style().bg
}

pub fn fg(screen: &Screen, row: u16, col: u16) -> Color {
    screen.cell(row, col).unwrap().style().fg
}

/// How many frames the Preview draws, by their top left corners.
pub fn frames(screen: &Screen) -> usize {
    pane(screen).iter().map(|r| r.matches('┌').count()).sum()
}

pub fn dim(screen: &Screen, text: &str) -> bool {
    let (row, col) = at(screen, text);
    screen.cell(row, col).unwrap().style().dim
}
