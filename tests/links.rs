//! `o` labels the links the Preview window shows, and a label opens its URL
//! in the browser, here a recording stand-in for `open` and `xdg-open`.

#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::{
    fmt::Write,
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{Duration, Instant},
};

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, Stall},
    home::Home,
};
use tempfile::TempDir;
use termlens::{Color, Key, Screen, Terminal};

/// A directory ahead on `PATH` whose `open` and `xdg-open` write each URL
/// they are given to a file.
struct Browser {
    dir: TempDir,
}

impl Browser {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\n",
            dir.path().join("opened").display()
        );
        for name in ["open", "xdg-open"] {
            let path = dir.path().join(name);
            fs::write(&path, &script).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        Self { dir }
    }

    fn path(&self) -> PathBuf {
        let rest = std::env::var_os("PATH").unwrap();
        let mut paths = vec![self.dir.path().to_owned()];
        paths.extend(std::env::split_paths(&rest));
        std::env::join_paths(paths).unwrap().into()
    }

    fn opened(&self) -> Vec<String> {
        fs::read_to_string(self.dir.path().join("opened"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Waits for `urls` to be every URL opened so far.
    fn wait_for(&self, urls: &[&str]) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.opened() != urls {
            assert!(Instant::now() < deadline, "opened {:?}", self.opened());
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn postal(home: &Home, browser: &Browser) -> Terminal {
    Terminal::builder()
        .size(120, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("PATH", browser.path())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn html(subject: &str, day: u32, html: &str) -> FakeMessage {
    let date = format!("2024-11-{day:02}T09:00:00Z");
    FakeMessage::new("Ada Lovelace", subject, &date).body(FakeBody::text("html", "7bit", html))
}

fn start(messages: Vec<FakeMessage>) -> (FakeImap, Home, Browser) {
    let server = FakeImap::start(messages);
    let home = Home::for_server(&server);
    (server, home, Browser::new())
}

const NOTES: &str = "https://example.com/notes";
const AGENDA: &str = "https://example.com/agenda";

/// Two web links, one of them twice, and a link that must not be opened.
fn two_links() -> String {
    format!(
        "<p>Read <a href=\"{NOTES}\">the notes</a> and <a href=\"{AGENDA}\">the agenda</a>.</p>\
         <p>Again: <a href=\"{NOTES}\">notes</a>. Also <a href=\"file:///etc/passwd\">a file</a>.</p>"
    )
}

/// The style of the first cell of `needle` on screen.
fn style_at(screen: &Screen, needle: &str) -> termlens::Style {
    (0..screen.rows())
        .find_map(|row| {
            let col = screen.row_text(row).find(needle)?;
            let col = u16::try_from(screen.row_text(row)[..col].chars().count()).unwrap();
            Some(*screen.cell(row, col).unwrap().style())
        })
        .unwrap()
}

// One label per distinct web URL, before its first text only; the URL of
// the label typed is the one opened.
#[test]
fn a_one_letter_label_opens_its_url() {
    let (_server, home, browser) = start(vec![html("Two links", 18, &two_links())]);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("│ Read the notes and the agenda"))
        .unwrap();

    t.send(Key::Char('o')).unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("Read athe notes and sthe agenda"))
        .unwrap();
    assert!(
        screen.contains("Again: notes. Also a file."),
        "{}",
        screen.text()
    );
    let label = style_at(&screen, "athe notes");
    assert_eq!(label.fg, Color::Indexed(3));
    assert!(label.reverse && label.bold, "{label:?}");

    t.send(Key::Char('s')).unwrap();
    browser.wait_for(&[AGENDA]);
    t.wait_until(|s| s.contains("Read the notes and the agenda"))
        .unwrap();
    quit(t);
}

// Past nine links the labels take two letters, all of them, so no label
// is the start of another.
#[test]
fn a_two_letter_label_opens_its_url() {
    let mut body = String::new();
    for i in 0..12 {
        let _ = write!(body, "<a href=\"https://example.com/{i}\">link {i}</a><br>");
    }

    let (_server, home, browser) = start(vec![html("Twelve links", 18, &body)]);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("link 11")).unwrap();

    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("aalink 0") && s.contains("sdlink 11"))
        .unwrap();
    t.send(Key::Char('s')).unwrap();
    t.send(Key::Char('d')).unwrap();
    browser.wait_for(&["https://example.com/11"]);
    t.wait_until(|s| s.contains("link 0") && !s.contains("aalink 0"))
        .unwrap();
    quit(t);
}

#[test]
fn links_scrolled_out_of_view_get_no_label() {
    let filler = "<br>filler".repeat(60);
    let body = format!(
        "<a href=\"https://example.com/top\">top link</a>{filler}<br>\
         <a href=\"https://example.com/bottom\">bottom link</a>"
    );
    let (_server, home, browser) = start(vec![html("Long", 18, &body)]);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("│ top link")).unwrap();

    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("atop link")).unwrap();
    t.send(Key::Char('a')).unwrap();
    browser.wait_for(&["https://example.com/top"]);

    for _ in 0..6 {
        t.send(Key::PageDown).unwrap();
    }

    t.wait_until(|s| s.contains("│ bottom link") && !s.contains("│ top link"))
        .unwrap();
    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("abottom link")).unwrap();
    t.send(Key::Char('a')).unwrap();
    browser.wait_for(&["https://example.com/top", "https://example.com/bottom"]);
    quit(t);
}

// A key that is no label leaves hint mode and does nothing else, even one
// the main screen binds.
#[test]
fn a_key_that_is_no_label_cancels_and_is_consumed() {
    let messages = vec![
        html("Older", 17, "<p>Nothing to see</p>"),
        html("Two links", 18, &two_links()),
    ];
    let (_server, home, browser) = start(messages);
    let mut t = postal(&home, &browser);
    let labelled = |s: &Screen| s.contains("athe notes");
    let unlabelled = |s: &Screen| s.contains(" 1/2 ") && s.contains("│ Read the notes");
    t.wait_until(unlabelled).unwrap();

    // `j` is in the label alphabet, but no label here is `j`.
    for cancel in [Key::Char('j'), Key::Esc, Key::Char('q')] {
        t.send(Key::Char('o')).unwrap();
        t.wait_until(labelled).unwrap();
        t.send(cancel).unwrap();
        t.wait_until(unlabelled).unwrap();
    }

    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains(" 2/2 ")).unwrap();
    quit(t);
    assert!(browser.opened().is_empty(), "{:?}", browser.opened());
}

// Labels belong to the Preview they were found in: once the selected
// Message is expunged under them, typing one must not open its old URL.
#[test]
fn hint_mode_ends_when_the_selected_message_goes() {
    let messages = vec![
        html(
            "Older",
            17,
            "<p><a href=\"https://example.com/older\">older</a></p>",
        ),
        html("Two links", 18, &two_links()),
    ];
    let (server, home, browser) = start(messages);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("│ Read the notes")).unwrap();
    quit(t);

    server.stall(Stall::Greeting);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("│ Read the notes")).unwrap();
    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("athe notes")).unwrap();
    server.expunge(FIRST_UID + 1);
    server.release();
    t.wait_until(|s| s.contains(" 1/1 ") && s.contains("│ older"))
        .unwrap();

    t.send(Key::Char('a')).unwrap();
    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("aolder")).unwrap();
    t.send(Key::Char('a')).unwrap();
    browser.wait_for(&["https://example.com/older"]);
    quit(t);
}

#[test]
fn no_visible_links_toasts_no_links() {
    let (_server, home, browser) = start(vec![html("Plain", 18, "<p>Nothing to see</p>")]);
    let mut t = postal(&home, &browser);
    t.wait_until(|s| s.contains("Nothing to see")).unwrap();
    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("no links")).unwrap();
    quit(t);
}
