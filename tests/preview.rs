#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, Outage, Stall},
    home::Home,
    quit, wait_for,
};
use termlens::{Key, Screen, Terminal};

/// What a plain Message's body is fetched by: its header and its one part.
const BODY: &str = "(UID BODY.PEEK[HEADER] BODY.PEEK[1])";

fn postal(home: &Home, cols: u16) -> Terminal {
    home.terminal()
        .size(cols, 30)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn idling(server: &FakeImap) -> bool {
    server.commands().iter().any(|c| c == "IDLE")
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
        message("Ada Lovelace", "Weekly notes", 17, notes)
            .to("Charles Babbage", "charles@example.com")
            .cc("Mary Somerville", "mary@example.com"),
    ]
}

fn notes_shown(screen: &Screen) -> bool {
    screen.contains("Bernoulli program") && screen.contains("Hi all, Here are")
}

// The layout from the main screen prototype: side by side from 84 columns,
// stacked below; the quote prefix survives wrapping. The header names who
// wrote to whom, `To` then `Cc`, with the date at the right.
#[test]
fn shows_the_preview_beside_or_below_the_list() {
    let server = FakeImap::start(kinds());
    let home = Home::for_server(&server);
    for (name, cols) in [("preview_120", 120), ("preview_80", 80)] {
        server.clear_commands();
        let mut t = postal(&home, cols);
        wait_for(|| idling(&server));
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
    numbered(6)
}

/// Messages from day 1 to day `days`, each body naming its day.
fn numbered(days: u32) -> Vec<FakeMessage> {
    (1..=days)
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
    format!("UID FETCH {uids} {BODY}")
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
    let fetches: Vec<_> = commands.iter().filter(|c| c.contains(BODY)).collect();
    assert_eq!(
        fetches,
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
    wait_for(|| idling(&server));
    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| previewed(s, 1)).unwrap();
    quit(t);
    let commands = server.commands();
    assert!(
        !commands.iter().any(|c| c.contains("BODY.PEEK[HEADER]")),
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
    server.stall(Stall::Body);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| s.contains("Loading…") && s.contains(" 1/6 "))
        .unwrap();
    t.send(Key::Char('j')).unwrap();
    // The header comes from the row, so it shows before the body.
    t.wait_until(|s| {
        s.contains("Loading…") && s.contains(" 2/6 ") && s.contains("Subject Subject 5")
    })
    .unwrap();

    server.release();
    t.wait_until(|s| previewed(s, 5)).unwrap();
    quit(t);
}

// Scrolling past rows whose bodies are not in yet must not queue them: the
// selection comes next, and rows scrolled past are never fetched.
#[test]
fn scrolling_past_unfetched_rows_fetches_only_where_it_stops() {
    let server = FakeImap::start(numbered(30));
    server.stall(Stall::Body);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| s.contains("Loading…") && s.contains(" 1/30 "))
        .unwrap();
    // The placeholder shows before the fetch connection is up; scrolling
    // first would leave row 1 never asked for.
    wait_for(|| server.commands().contains(&uid_fetch("130")));
    for row in 2..=6 {
        t.send(Key::Char('j')).unwrap();
        t.wait_until(|s| s.contains(&format!(" {row}/30 ")))
            .unwrap();
    }

    server.release();
    t.wait_until(|s| previewed(s, 25)).unwrap();
    wait_for(|| bodies(&home) == 6);
    quit(t);

    let commands = server.commands();
    let fetches: Vec<_> = commands.iter().filter(|c| c.contains(BODY)).collect();
    assert_eq!(
        fetches[..2],
        [&uid_fetch("130"), &uid_fetch("125")],
        "{commands:#?}"
    );
    assert!(
        !fetches
            .iter()
            .any(|c| c.contains("128") || c.contains("129")),
        "{commands:#?}"
    );
}

/// The first row of the Preview's text at 80 columns, 30 rows.
fn top(screen: &Screen) -> String {
    screen.row_text(14).trim_matches(['│', ' ']).to_owned()
}

/// Waits for `expected` on the body's first row, under a header that
/// stays put.
fn wait_top(t: &mut Terminal, expected: &str) {
    t.wait_until(|s| {
        let body = s.row_text(18).trim_matches(['│', ' ']).to_owned();
        top(s).starts_with("From") && body == expected
    })
    .unwrap();
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

    // 14 rows of text: half a page is 7, scrolling the 10 body rows under
    // the header and its rule.
    t.send(Key::Ctrl('d')).unwrap();
    wait_top(&mut t, "Line 8");
    t.send(Key::PageDown).unwrap();
    wait_top(&mut t, "Line 15");
    t.send(Key::Ctrl('u')).unwrap();
    wait_top(&mut t, "Line 8");
    // Scrolling stops with the last line at the bottom.
    for _ in 0..10 {
        t.send(Key::Ctrl('d')).unwrap();
    }

    wait_top(&mut t, "Line 31");
    t.send(Key::PageUp).unwrap();
    wait_top(&mut t, "Line 24");

    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("Short body")).unwrap();
    t.send(Key::Char('k')).unwrap();
    t.wait_until(|s| top(s).starts_with("From") && s.contains("Line 1"))
        .unwrap();
    quit(t);
}

fn hint_line(screen: &Screen) -> String {
    screen.row_text(screen.rows() - 1)
}

fn logins(server: &FakeImap) -> usize {
    let commands = server.commands();
    commands
        .iter()
        .filter(|c| c.starts_with("AUTHENTICATE"))
        .count()
}

// With the server gone, a body not in the Cache says so under the row's
// header, and comes by itself, with its neighbours, once the server is back.
#[test]
fn a_body_missing_while_offline_says_so_and_comes_after_the_reconnect() {
    let server = FakeImap::start(six());
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| previewed(s, 6)).unwrap();
    wait_for(|| bodies(&home) == 3);

    // The fake drops connections on a tick: until then a fetch still succeeds.
    server.fail(Outage::Offline);
    t.wait_until(|s| hint_line(s).contains("offline · retry in"))
        .unwrap();
    t.send(Key::Char('G')).unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("Not downloaded yet · offline"))
        .unwrap();
    assert!(
        screen.contains("From    Sender <someone@example.com>"),
        "{screen}"
    );
    assert!(screen.contains("Subject Subject 1"), "{screen}");
    assert!(screen.contains("Fri, 1 Nov 2024 09:00"), "{screen}");

    t.send(Key::Char('|')).unwrap();
    t.wait_until(|s| hint_line(s).contains("not downloaded"))
        .unwrap();
    server.recover();
    t.wait_until(|s| previewed(s, 1)).unwrap();
    wait_for(|| bodies(&home) == 6);
    quit(t);
}

// A body the server refuses says why, and only that fetch fails: the
// neighbours still come on the same connection.
#[test]
fn a_refused_body_says_why_and_keeps_the_connection() {
    let server = FakeImap::start(six());
    server.refuse_body(FIRST_UID + 2);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| previewed(s, 6)).unwrap();
    wait_for(|| bodies(&home) == 3);
    let before = logins(&server);

    for _ in 0..3 {
        t.send(Key::Char('j')).unwrap();
    }

    let screen = t
        .snapshot_after(|s| s.contains(" 4/6 ") && s.contains("Couldn't load: Some messages"))
        .unwrap();
    assert!(screen.contains("Subject Subject 3"), "{screen}");
    wait_for(|| bodies(&home) == 5);
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| previewed(s, 2)).unwrap();
    quit(t);

    assert_eq!(logins(&server), before);
    let log = std::fs::read_to_string(home.log_path()).unwrap();
    let refused = "Some messages could not be FETCHed (Failure)";
    assert!(
        log.lines()
            .any(|l| l.contains("WARN") && l.contains("refused") && l.contains(refused)),
        "{log}"
    );
    // Its Snippet is refused too, and that cost no connection either.
    assert!(!log.contains("retrying in"), "{log}");
}

// One refused body among the neighbours costs only that one: the others
// asked for in the same command still come.
#[test]
fn a_refused_neighbour_leaves_the_rest_of_its_batch_loadable() {
    let server = FakeImap::start(six());
    server.refuse_body(FIRST_UID + 2);
    let home = Home::for_server(&server);
    let mut t = postal(&home, 120);
    t.wait_until(|s| previewed(s, 6)).unwrap();
    wait_for(|| bodies(&home) == 3);

    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| previewed(s, 1)).unwrap();
    t.send(Key::Char('k')).unwrap();
    t.wait_until(|s| s.contains(" 5/6 ") && previewed(s, 2))
        .unwrap();
    t.send(Key::Char('k')).unwrap();
    t.wait_until(|s| s.contains(" 4/6 ") && s.contains("Couldn't load: Some messages"))
        .unwrap();
    quit(t);
}
