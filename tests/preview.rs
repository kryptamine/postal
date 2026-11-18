#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::time::{Duration, Instant};

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, Stall},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

const WHOLE: &str = "(UID BODY.PEEK[])";

fn postal(home: &Home, cols: u16) -> Terminal {
    Terminal::builder()
        .size(cols, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn logged_out(server: &FakeImap) -> bool {
    server.commands().iter().any(|c| c == "LOGOUT")
}

fn message(sender: &'static str, subject: &str, day: u32, body: FakeBody) -> FakeMessage {
    FakeMessage::new(sender, subject, &format!("2024-11-{day:02}T09:00:00Z")).body(body)
}

const NOTES: &str = "Hi all,\r\n\r\nHere are this week's notes. The analytical engine \
    prototype now runs the full Bernoulli program without a single jam, which is a first.\r\n\
    \r\n\
    > > Can it also print the table it computes, or only show it on the dials? A printed \
    copy would save a great deal of copying by hand.\r\n\
    > Yes: the printer arrived on Tuesday.\r\n\
    \r\n\
    Thanks,\r\n\
    Ada\r\n\r\n\r\n";

/// The newest has a plain part and an attachment.
fn kinds() -> Vec<FakeMessage> {
    let notes = FakeBody::multipart(
        "mixed",
        vec![FakeBody::plain(NOTES), FakeBody::file("application", "pdf")],
    );
    vec![
        message(
            "Charles",
            "Only attachments",
            15,
            FakeBody::multipart("mixed", vec![FakeBody::file("image", "png")]),
        ),
        message(
            "Newsletter",
            "Html only",
            16,
            FakeBody::text("html", "7bit", "<p>Only markup here</p>"),
        ),
        message("Ada Lovelace", "Weekly notes", 17, notes),
    ]
}

fn notes_shown(screen: &Screen) -> bool {
    screen.contains("Bernoulli program") && screen.contains("Hi all, Here are")
}

// The layout from the main screen prototype: side by side from 84 columns,
// stacked below; the quote prefix survives wrapping.
#[test]
fn shows_the_preview_beside_or_below_the_list() {
    let server = FakeImap::start(kinds());
    let home = Home::for_server(&server);
    for (name, cols) in [("preview_120", 120), ("preview_80", 80)] {
        server.clear_commands();
        let mut t = postal(&home, cols);
        wait_for(|| logged_out(&server));
        let screen = t.snapshot_after(notes_shown).unwrap();
        insta::assert_snapshot!(name, screen);
        quit(t);
    }
}

#[test]
fn an_html_only_message_shows_its_html_and_one_without_text_says_so() {
    let server = FakeImap::start(kinds());
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(notes_shown).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.row_text(5).contains("Only markup here"))
        .unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("no text")).unwrap();
    quit(t);
}

/// UIDs 101 to 106, the newest last, so the list shows 106 first.
fn six() -> Vec<FakeMessage> {
    (1..=6)
        .map(|day| {
            let body = FakeBody::plain(&format!("Body number {day}"));
            message("Sender", &format!("Subject {day}"), day, body)
        })
        .collect()
}

/// Whether the Preview at 120 columns shows Message `n` of `six`, not
/// only its Snippet in the list.
fn previewed(screen: &Screen, n: u32) -> bool {
    screen.row_text(5).contains(&format!("Body number {n}"))
}

fn bodies(home: &Home) -> i64 {
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    cache
        .query_row("SELECT count(*) FROM body", [], |row| row.get(0))
        .unwrap()
}

fn uid_fetch(uids: &str) -> String {
    format!("UID FETCH {uids} {WHOLE}")
}

#[test]
fn selecting_a_row_fetches_it_and_its_neighbours() {
    let server = FakeImap::start(six());
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| previewed(s, 6)).unwrap();
    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| previewed(s, 1)).unwrap();
    wait_for(|| bodies(&home) == 6);
    quit(t);

    // The selection is fetched alone, so it never waits on its neighbours.
    let commands = server.commands();
    let wholes: Vec<_> = commands.iter().filter(|c| c.contains(WHOLE)).collect();
    assert_eq!(
        wholes,
        [
            &uid_fetch("106"),
            &uid_fetch("104,105"),
            &uid_fetch("101"),
            &uid_fetch("102,103"),
        ],
        "{commands:#?}"
    );

    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let mut statement = cache.prepare("SELECT key FROM body ORDER BY key").unwrap();
    let keys: Vec<i64> = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let msgids: Vec<_> = (FIRST_UID..FIRST_UID + 6)
        .map(|uid| server.msgid(uid).cast_signed())
        .collect();
    assert_eq!(keys, msgids);

    // A second run shows every body from the Cache.
    server.clear_commands();
    let mut t = postal(&home, 120);
    t.wait_until(|s| previewed(s, 6)).unwrap();
    wait_for(|| logged_out(&server));
    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| previewed(s, 1)).unwrap();
    quit(t);
    let commands = server.commands();
    assert!(
        !commands.iter().any(|c| c.contains("BODY.PEEK[]")),
        "{commands:#?}"
    );
}

#[test]
fn a_server_without_gmail_extensions_keys_bodies_by_uid() {
    let server = FakeImap::start(six());
    server.without_gmail();
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| previewed(s, 6)).unwrap();
    quit(t);

    let commands = server.commands();
    assert!(
        !commands.iter().any(|c| c.contains("X-GM-MSGID")),
        "{commands:#?}"
    );
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let key: i64 = cache
        .query_row("SELECT body_key FROM message WHERE uid = 106", [], |row| {
            row.get(0)
        })
        .unwrap();
    // UIDVALIDITY 1 in the high 32 bits, the UID in the low ones.
    assert_eq!(key, (1 << 32) + 106);
}

#[test]
fn a_preview_not_yet_fetched_shows_a_placeholder_and_never_blocks() {
    let server = FakeImap::start(six());
    server.stall(Stall::Whole);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| s.contains("Loading…") && s.contains(" 1/6 "))
        .unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("Loading…") && s.contains(" 2/6 "))
        .unwrap();

    server.release();
    t.wait_until(|s| previewed(s, 5)).unwrap();
    quit(t);
}

/// The first row of the Preview's text at 80 columns, 30 rows.
fn top(screen: &Screen) -> String {
    screen.row_text(14).trim_matches(['│', ' ']).to_owned()
}

fn wait_top(t: &mut Terminal, expected: &str) {
    t.wait_until(|s| top(s) == expected).unwrap();
}

#[test]
fn scrolls_half_a_page_and_back_to_the_top_on_another_message() {
    let lines: Vec<_> = (1..=40).map(|n| format!("Line {n}")).collect();
    let long = FakeBody::plain(&lines.join("\r\n"));
    let server = FakeImap::start(vec![
        message("Sender", "Short", 1, FakeBody::plain("Short body")),
        message("Sender", "Long", 2, long),
    ]);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 80);
    t.wait_until(|s| top(s).starts_with("From") && s.row_text(18).contains("Line 1"))
        .unwrap();

    // 15 rows of text: half a page is 7, past the 4 header rows.
    t.send(Key::Ctrl('d')).unwrap();
    wait_top(&mut t, "Line 4");
    t.send(Key::PageDown).unwrap();
    wait_top(&mut t, "Line 11");
    t.send(Key::Ctrl('u')).unwrap();
    wait_top(&mut t, "Line 4");
    // Scrolling stops with the last line at the bottom.
    for _ in 0..10 {
        t.send(Key::Ctrl('d')).unwrap();
    }

    wait_top(&mut t, "Line 26");
    t.send(Key::PageUp).unwrap();
    wait_top(&mut t, "Line 19");

    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("Short body")).unwrap();
    t.send(Key::Char('k')).unwrap();
    t.wait_until(|s| top(s).starts_with("From") && s.contains("Line 1"))
        .unwrap();
    quit(t);
}
