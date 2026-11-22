//! `Enter` reads the seleted Thread full screen as a stack of Messages,
//! with keys of its own, until `q` or `Esc` goes back to the list.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::time::Duration;

use support::{
    browser::Browser,
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

fn postal(home: &Home) -> Terminal {
    Terminal::builder()
        .size(100, 16)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

/// Forty numbered lines, more than one screen holds.
fn long_body() -> FakeBody {
    let lines: Vec<String> = (1..=40).map(|n| format!("Line {n:02}")).collect();
    FakeBody::plain(&lines.join("\r\n"))
}

fn two_messages() -> Vec<FakeMessage> {
    vec![
        FakeMessage::new("Ada", "Older", "2024-11-17T09:00:00Z")
            .body(FakeBody::plain("Older body")),
        FakeMessage::new("Bob", "Newer", "2024-11-18T09:00:00Z").body(long_body()),
    ]
}

fn listed(screen: &Screen) -> bool {
    screen.contains(" Inbox ") && screen.contains("Line 01")
}

fn reading(screen: &Screen) -> bool {
    !screen.contains(" Inbox ") && screen.row_text(screen.rows() - 1).contains("q back")
}

// The list gives way to the Message, and j/k move through it, not through
// the list: the next Message never shows.
#[test]
fn enter_reads_the_message_full_screen_and_q_goes_back() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(listed).unwrap();

    t.send(Key::Enter).unwrap();
    let screen = t.snapshot_after(reading).unwrap();
    assert!(screen.contains("From    Bob"), "{screen}");

    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("Line 03") && !s.contains("Line 01"))
        .unwrap();
    t.send(Key::Char('G')).unwrap();
    let screen = t.snapshot_after(|s| s.contains("Line 40")).unwrap();
    assert!(!screen.contains("Older body"), "{screen}");
    t.send(Key::Char('g')).unwrap();
    t.wait_until(|s| s.contains("From    Bob")).unwrap();

    // `q` leaves the Message, not Postal.
    t.send(Key::Char('q')).unwrap();
    t.wait_until(listed).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(reading).unwrap();
    t.send(Key::Esc).unwrap();
    t.wait_until(listed).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

// The full-screen Message belongs to the Message it was opened on: once
// that one is gone, the list comes back on the one selected after it.
#[test]
fn reading_ends_when_the_message_is_expunged() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(listed).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(reading).unwrap();

    server.expunge(FIRST_UID + 1);
    t.wait_until(|s| s.contains(" Inbox ") && s.contains("Older body"))
        .unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn the_help_popup_lists_the_reading_keys() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = Terminal::builder()
        .size(80, 24)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(|s| s.contains(" · 1/")).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(reading).unwrap();
    t.send(Key::Char('?')).unwrap();
    let screen = t.snapshot_after(|s| s.contains(" Help ")).unwrap();
    insta::assert_snapshot!(screen);
    t.send(Key::Esc).unwrap();
    // A key read with `Esc` in one go is taken for Alt and the Esc is lost.
    t.wait_until(|s| !s.contains(" Help ")).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// Postal at 30 rows, past its first frame: a key sent before it goes with
/// the answer to the colour query.
fn tall(home: &Home, browser: &Browser) -> Terminal {
    let mut t = Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("PATH", browser.path())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(|s| s.contains(" Inbox ")).unwrap();
    t
}

fn reply(sender: &'static str, body: &str, day: u32) -> FakeMessage {
    let date = format!("2024-03-{day:02}T09:00:00Z");
    FakeMessage::new(sender, "Plans", &date)
        .body(FakeBody::plain(body))
        .in_thread(0)
}

/// One Thread: Ada asks, Bob answers, Unread when `bob_unread`, Cid agrees
/// and Dee settles it.
fn conversation(bob_unread: bool) -> Vec<FakeMessage> {
    let bob = reply("Bob", "Bob answers", 2);
    vec![
        reply("Ada", "Ada asks", 1),
        if bob_unread { bob.unread() } else { bob },
        reply("Cid", "Cid agrees", 3),
        reply("Dee", "Dee settles it", 4),
    ]
}

/// Whether a row shows `sender` folded: its name, Snippet and date.
fn folded(screen: &Screen, sender: &str, snippet: &str, date: &str) -> bool {
    (0..screen.rows()).any(|row| {
        let text = screen.row_text(row);
        !text.contains("From")
            && text.contains(sender)
            && text.contains(snippet)
            && text.contains(date)
    })
}

/// Opens the Thread once the list shows it with the newest Snippet.
fn read_thread(t: &mut Terminal) {
    t.wait_until(|s| s.contains("Dee settles it")).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(reading).unwrap();
}

/// The first row that shows `text`.
fn row_of(screen: &Screen, text: &str) -> u16 {
    (0..screen.rows())
        .find(|&row| screen.row_text(row).contains(text))
        .unwrap_or_else(|| panic!("no {text}: {screen}"))
}

// On entry the newest and every Unread Message are open, the rest one line
// each, and the cursor marks the oldest Unread.
#[test]
fn reading_unfolds_the_newest_and_the_unread_and_starts_on_the_oldest_unread() {
    let server = FakeImap::start(conversation(true));
    let home = Home::for_server(&server);
    let browser = Browser::new();
    let mut t = tall(&home, &browser);
    read_thread(&mut t);

    let screen = t
        .snapshot_after(|s| {
            s.contains("Bob answers")
                && s.contains("Dee settles it")
                && folded(s, "Ada", "Ada asks", "2024-03-01")
                && folded(s, "Cid", "Cid agrees", "2024-03-03")
        })
        .unwrap();
    assert!(screen.contains(" 2/4 ┐"), "{screen}");
    // The border names the Thread once, so no Message repeats its subject.
    assert!(screen.contains("┌ Plans · Ada, Bob, Cid, Dee "), "{screen}");
    assert!(!screen.contains("Subject"), "{screen}");
    assert!(screen.contains("│▎From    Bob"), "{screen}");
    assert!(screen.contains("│ From    Dee"), "{screen}");
    assert!(!screen.contains("From    Ada"), "{screen}");
    assert!(!screen.contains("From    Cid"), "{screen}");
    // One order from the top down, the way the Messages came, so the top
    // is never taken for the latest.
    let rows = ["Ada asks", "From    Bob", "Cid agrees", "From    Dee"].map(|t| row_of(&screen, t));
    assert!(rows.is_sorted(), "{rows:?}: {screen}");
    quit(t);
}

#[test]
fn with_nothing_unread_only_the_newest_is_unfolded_and_has_the_cursor() {
    let server = FakeImap::start(conversation(false));
    let home = Home::for_server(&server);
    let browser = Browser::new();
    let mut t = tall(&home, &browser);
    read_thread(&mut t);

    let screen = t
        .snapshot_after(|s| {
            s.contains("│▎From    Dee") && folded(s, "Ada", "Ada asks", "2024-03-01")
        })
        .unwrap();
    assert!(screen.contains(" 4/4 "), "{screen}");
    assert!(!screen.contains("From    Bob"), "{screen}");
    quit(t);
}

// The read middle of a Thread takes one line, so the newest is not a
// screen of old ones away; Enter puts each of them on its own line.
#[test]
fn the_folded_middle_of_a_thread_is_one_line_until_opened() {
    let server = FakeImap::start(conversation(false));
    let home = Home::for_server(&server);
    let browser = Browser::new();
    let mut t = tall(&home, &browser);
    read_thread(&mut t);

    let screen = t
        .snapshot_after(|s| s.contains("│▎From    Dee") && s.contains("2 earlier messages"))
        .unwrap();
    assert!(!screen.contains("Bob answers"), "{screen}");
    assert!(!screen.contains("Cid agrees"), "{screen}");
    let rows = ["Ada asks", "2 earlier messages", "From    Dee"].map(|t| row_of(&screen, t));
    assert!(rows.is_sorted(), "{rows:?}: {screen}");

    t.send(Key::Char('p')).unwrap();
    t.wait_until(|s| s.contains("│▎··· 2 earlier messages") && s.contains(" 2/4 "))
        .unwrap();
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| {
            folded(s, "Bob", "Bob answers", "2024-03-02")
                && folded(s, "Cid", "Cid agrees", "2024-03-03")
        })
        .unwrap();
    assert!(screen.contains("│▎Bob"), "{screen}");
    assert!(!screen.contains("earlier messages"), "{screen}");
    t.send(Key::Char('n')).unwrap();
    t.wait_until(|s| s.contains("│▎Cid") && s.contains(" 3/4 "))
        .unwrap();
    quit(t);
}

// `n` and `p` move the cursor a Message at a time and stop at either end.
#[test]
fn n_and_p_move_the_cursor_through_the_thread() {
    let server = FakeImap::start(conversation(true));
    let home = Home::for_server(&server);
    let browser = Browser::new();
    let mut t = tall(&home, &browser);
    read_thread(&mut t);
    t.wait_until(|s| s.contains(" 2/4 ")).unwrap();

    for (key, title) in [
        ('p', " 1/4 "),
        ('p', " 1/4 "),
        ('n', " 2/4 "),
        ('n', " 3/4 "),
        ('n', " 4/4 "),
        ('n', " 4/4 "),
        ('p', " 3/4 "),
    ] {
        t.send(Key::Char(key)).unwrap();
        t.wait_until(|s| s.contains(title)).unwrap();
    }

    quit(t);
}

// Enter and Space fold the Message at the cursor and unfold it again; a
// folded one opens on its own body.
#[test]
fn enter_and_space_fold_and_unfold_the_message_at_the_cursor() {
    let server = FakeImap::start(conversation(true));
    let home = Home::for_server(&server);
    let browser = Browser::new();
    let mut t = tall(&home, &browser);
    read_thread(&mut t);
    t.wait_until(|s| s.contains("From    Bob")).unwrap();

    t.send(Key::Enter).unwrap();
    t.wait_until(|s| !s.contains("From    Bob") && folded(s, "Bob", "Bob answers", "2024-03-02"))
        .unwrap();
    t.send(Key::Char(' ')).unwrap();
    t.wait_until(|s| s.contains("│▎From    Bob")).unwrap();

    t.send(Key::Char('n')).unwrap();
    t.send(Key::Char(' ')).unwrap();
    let screen = t.snapshot_after(|s| s.contains("│▎Cid agrees")).unwrap();
    assert!(screen.contains("│▎From    Cid"), "{screen}");
    assert!(screen.contains("From    Bob"), "{screen}");
    quit(t);
}

/// Forty numbered lines, each led by `tag`.
fn numbered(tag: &str) -> FakeBody {
    let lines: Vec<String> = (1..=40).map(|n| format!("{tag} {n:02}")).collect();
    FakeBody::plain(&lines.join("\r\n"))
}

// A Message scrolled past its top keeps its header pinned over the window
// with a rule under it, as the Preview does; the body scrolls beneath.
#[test]
fn a_scrolled_message_keeps_its_header_on_top() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(listed).unwrap();
    t.send(Key::Enter).unwrap();
    // Until the body renders at Reading's width, laying it out clamps the
    // scroll to its placeholder, and the first `j`s are lost.
    t.wait_until(|s| reading(s) && s.contains("Line 01"))
        .unwrap();

    for _ in 0..10 {
        t.send(Key::Char('j')).unwrap();
    }

    let screen = t
        .snapshot_after(|s| s.row_text(4).contains("Line 11"))
        .unwrap();
    insta::assert_snapshot!(screen);
    quit(t);
}

fn long(sender: &'static str, day: u32) -> FakeMessage {
    let date = format!("2024-03-{day:02}T09:00:00Z");
    FakeMessage::new(sender, "Long", &date)
        .body(numbered(sender))
        .in_thread(0)
        .unread()
}

// The next Message pushes a pinned header up as the last rows of its own
// Message leave, so the two headers never overlap.
#[test]
fn the_next_message_pushes_a_pinned_header_up() {
    let server = FakeImap::start(vec![long("Ada", 1), long("Bob", 2)]);
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Bob 01")).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(" 1/2 ") && s.contains("Ada 01"))
        .unwrap();

    // Ada's 43 rows end two rows below the top: her header's last two.
    for _ in 0..41 {
        t.send(Key::Char('j')).unwrap();
    }

    let screen = t
        .snapshot_after(|s| s.row_text(1).contains("To") && s.row_text(4).contains("From    Bob"))
        .unwrap();
    insta::assert_snapshot!(screen);
    quit(t);
}

// j/k, ^d/^u and g/G scroll the stack as one page, from the end of one
// Message into the next, and the cursor follows to a Message still shown.
#[test]
fn scrolling_runs_across_messages_and_the_cursor_follows() {
    let server = FakeImap::start(vec![long("Ada", 1), long("Bob", 2)]);
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Bob 01")).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(" 1/2 ") && s.contains("Ada 01"))
        .unwrap();

    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| s.contains("Bob 40") && s.contains(" 2/2 "))
        .unwrap();
    t.send(Key::Char('g')).unwrap();
    t.wait_until(|s| s.contains("From    Ada") && s.contains(" 1/2 "))
        .unwrap();

    // 13 rows shown, 6 to a half page: Ada's last lines over Bob's header.
    for _ in 0..6 {
        t.send(Key::Ctrl('d')).unwrap();
    }

    let screen = t
        .snapshot_after(|s| s.contains("Ada 40") && s.contains("From    Bob"))
        .unwrap();
    assert!(screen.contains(" 1/2 "), "{screen}");
    for key in [
        Key::Char('j'),
        Key::Char('k'),
        Key::Ctrl('d'),
        Key::Ctrl('d'),
    ] {
        t.send(key).unwrap();
    }

    t.wait_until(|s| !s.contains("Ada 40") && s.contains(" 2/2 "))
        .unwrap();
    t.send(Key::Ctrl('u')).unwrap();
    t.send(Key::Ctrl('u')).unwrap();
    t.wait_until(|s| s.contains("Ada 40") && s.contains(" 2/2 "))
        .unwrap();
    quit(t);
}

// `o` labels the links of every unfolded body on screen as one set, and
// a label opens the URL of the Message it sits in.
#[test]
fn hint_mode_labels_links_across_two_unfolded_messages() {
    let html = |sender: &'static str, day: u32| {
        let date = format!("2024-03-{day:02}T09:00:00Z");
        let url = format!("https://example.com/{sender}");
        let body = format!("<p>See <a href=\"{url}\">the {sender} link</a>.</p>");
        FakeMessage::new(sender, "Links", &date)
            .body(FakeBody::text("html", "7bit", body))
            .in_thread(0)
            .unread()
    };

    let server = FakeImap::start(vec![html("Ada", 1), html("Bob", 2)]);
    let home = Home::for_server(&server);
    let browser = Browser::new();
    let mut t = tall(&home, &browser);
    // The text View moves a link right of its label.
    t.send(Key::Char('v')).unwrap();
    t.wait_until(|s| s.contains("See the Bob link")).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| {
        reading(s) && s.contains("See the Ada link") && s.contains("See the Bob link")
    })
    .unwrap();

    t.send(Key::Char('o')).unwrap();
    t.wait_until(|s| s.contains("See athe Ada link") && s.contains("See sthe Bob link"))
        .unwrap();
    t.send(Key::Char('s')).unwrap();
    browser.wait_for(&["https://example.com/Bob"]);
    t.wait_until(|s| s.contains("See the Bob link")).unwrap();
    quit(t);
}

// The Account's own Messages are muted, so a Thread with one person reads
// as a conversation and not as a column of From lines.
#[test]
fn the_accounts_own_messages_are_muted() {
    let server = FakeImap::start(vec![
        reply("Ada", "Ada asks", 1),
        reply("Ada", "I answer", 2).sent_by_me(),
        reply("Ada", "Dee settles it", 3),
    ]);
    let home = Home::for_server(&server);
    let browser = Browser::new();
    let mut t = tall(&home, &browser);
    read_thread(&mut t);

    let screen = t
        .snapshot_after(|s| {
            folded(s, "me", "I answer", "2024-03-02") && folded(s, "Ada", "Ada asks", "2024-03-01")
        })
        .unwrap();
    assert!(style_after(&screen, "I answer", "│ ").dim, "{screen}");
    assert!(!style_after(&screen, "Ada asks", "│ ").dim, "{screen}");

    t.send(Key::Char('p')).unwrap();
    t.send(Key::Char(' ')).unwrap();
    let screen = t
        .snapshot_after(|s| {
            s.contains("│▎From    ")
                && s.contains("I answer")
                && !folded(s, "me", "I answer", "2024-03-02")
        })
        .unwrap();
    let from = "│▎From    ";
    assert!(style_after(&screen, from, from).dim, "{screen}");
    quit(t);
}

/// The style of the cell right after `before` on the first row showing `text`.
fn style_after(screen: &Screen, text: &str, before: &str) -> termlens::Style {
    let row = row_of(screen, text);
    let line = screen.row_text(row);
    let (head, _) = line.split_once(before).unwrap();
    let col = head.chars().count() + before.chars().count();
    *screen
        .cell(row, u16::try_from(col).unwrap())
        .unwrap()
        .style()
}
