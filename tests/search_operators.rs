//! Search by operators, `from:ada is:unread`, and the menu over the Query
//! line that completes them.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::time::{Duration, Instant};

use support::{
    fake_body::FakeBody,
    fake_imap::{EMAIL, FIRST_UID, FakeImap, FakeMessage, INBOX},
    home::Home,
    search::{
        hint_line, list, message, messages, postal, quit, ready, selected_subject, show,
        wait_selected,
    },
};
use termlens::{Key, Screen, Terminal};

/// A Search reads this many candidates at most.
const LIMIT: u32 = 500;

/// Ada's unread notes with a file, Grace's draft to Petr and Olga, and
/// Petr's invoice, newest first; each Snippet holds `engine`.
fn people() -> Vec<FakeMessage> {
    let mut ada = FakeMessage::new("Ada", "Engine notes", "2024-04-01T09:00:00Z")
        .body(FakeBody::multipart(
            "mixed",
            vec![
                FakeBody::plain("Bernoulli numbers for the engine"),
                FakeBody::file("application", "pdf").disposition_param("filename", "notes.pdf"),
            ],
        ))
        .unread();
    ada.from_address = "ada@lovelace.org";
    let mut grace = message(
        "Grace",
        "Compiler draft",
        "2024-02-01T09:00:00Z",
        "Engine of the compiler, invoice later",
    )
    .to("Petr Novak", "p.novak@example.org")
    .cc("Olga Smirnova", "olga@example.org");
    grace.from_address = "grace@navy.mil";
    let mut petr = message(
        "Petr Novak",
        "Invoice 1042",
        "2023-06-01T09:00:00Z",
        "Pay engine invoice",
    );
    petr.from_address = "p.novak@example.org";
    vec![ada, grace, petr]
}

const SUBJECTS: [&str; 3] = ["Engine notes", "Compiler draft", "Invoice 1042"];

/// Types `query` into the Search under way, waits until the list holds
/// exactly the subjects `found` of `SUBJECTS`, then erases it.
fn finds(t: &mut Terminal, query: &str, found: &[&str]) {
    t.send_str(query).unwrap();
    let shown = |s: &Screen| {
        let list = list(s);
        let shown: Vec<_> = SUBJECTS.into_iter().filter(|s| list.contains(s)).collect();
        hint_line(s).contains(query) && shown == found
    };
    let none = found.is_empty();
    let screen = t
        .snapshot_after(|s| shown(s) && (!none || s.contains("no matches")))
        .unwrap_or_else(|e| panic!("{query}: {e}"));
    assert!(shown(&screen), "{query}\n{screen}");
    for _ in query.chars() {
        t.send(Key::Backspace).unwrap();
    }

    t.wait_until(|s| hint_line(s).trim() == "/").unwrap();
}

#[test]
fn each_operator_narrows_the_search_alone_and_with_words() {
    let server = FakeImap::start(people());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Bernoulli")).unwrap();

    t.send(Key::Char('/')).unwrap();
    let [notes, draft, invoice] = SUBJECTS;
    for (query, found) in [
        ("from:ada", &[notes][..]),
        ("from:lovelace", &[notes]),
        // A recipient of mail someone else sent, which a bare word misses.
        ("to:novak", &[draft]),
        ("cc:smirnova", &[draft]),
        ("subject:invoice", &[invoice]),
        ("invoice", &[draft, invoice]),
        ("engine from:grace", &[draft]),
        ("is:unread", &[notes]),
        ("is:read", &[draft, invoice]),
        ("has:attachment", &[notes]),
        ("before:2024-01-01", &[invoice]),
        ("after:2024-03-01", &[notes]),
        ("older:1d", &[notes, draft, invoice]),
        ("newer:7d", &[]),
        ("engine -from:ada", &[draft, invoice]),
        ("-compiler", &[notes, invoice]),
        ("-is:unread -has:attachment", &[draft, invoice]),
        ("\"engine invoice\"", &[invoice]),
        ("engine invoice", &[draft, invoice]),
        // An unknown operator is searched as words.
        ("invoice:1042", &[invoice]),
    ] {
        finds(&mut t, query, found);
    }

    quit(t);
}

#[test]
fn a_narrow_query_is_not_cut_by_the_newest_matches() {
    let mut inbox = messages();
    inbox.push(message("News", "Weekly digest", "2023-11-15T09:00:00Z", "Edition 3").unread());
    let server = FakeImap::start(inbox);
    server.add_folder("Lists", &[], vec![]);
    server.add_folder("Trash", &["\\Trash"], vec![]);
    let home = Home::for_server(&server);
    quit(ready(&server, &home));

    // Written straight into the Cache, all newer than any Message above.
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let account = EMAIL.to_lowercase();
    let row = "INSERT INTO message (account, mailbox, uid, body_key, received,
          received_offset, unread, answered, sender, from_line, to_line, subject,
          from_me, snippet, attachments)
        VALUES (?1, ?2, ?3, ?3, 1800000000 + ?3, 0, 0, 0, 'List', '', '', ?4, 0, '', '')";
    for uid in 1..=LIMIT {
        cache
            .execute(
                row,
                rusqlite::params![account, "Lists", uid, "Daily digest"],
            )
            .unwrap();
    }

    let trashed = rusqlite::params![account, "Trash", LIMIT + 1, "Trashed digest"];
    cache.execute(row, trashed).unwrap();
    drop(cache);

    // Until the Folders are listed, a Search reads no `Lists`, and the
    // Unread digest selected as the best match would be read.
    let mut t = ready(&server, &home);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !std::fs::read_to_string(home.log_path())
        .is_ok_and(|log| log.contains("listed 3 Folders"))
    {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }

    t.send(Key::Char('/')).unwrap();
    t.send_str("digest").unwrap();
    t.wait_until(|s| hint_line(s).contains("type more"))
        .unwrap();
    for _ in 0.."digest".len() {
        t.send(Key::Backspace).unwrap();
    }

    for (query, shown) in [
        ("digest is:unread", "Edition 3"),
        ("digest before:2024-01-01", "Edition 2"),
        ("digest in:trash", "Trashed digest"),
    ] {
        t.send_str(query).unwrap();
        let screen = t
            .snapshot_after(|s| hint_line(s).contains(query) && list(s).contains(shown))
            .unwrap();
        assert!(!list(&screen).contains("Daily digest"), "{screen}");
        assert!(!hint_line(&screen).contains("type more"), "{screen}");
        for _ in query.chars() {
            t.send(Key::Backspace).unwrap();
        }

        t.wait_until(|s| hint_line(s).trim() == "/").unwrap();
    }

    quit(t);
}

/// Whether the Query ends in `end`.
fn ends(screen: &Screen, end: &str) -> bool {
    hint_line(screen).trim_end().ends_with(end)
}

#[test]
fn the_menu_suggests_what_the_last_word_can_become() {
    let server = FakeImap::start(people());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Bernoulli")).unwrap();

    // Every operator for an empty Query.
    t.send(Key::Char('/')).unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("by name or address"))
        .unwrap();
    for operator in [
        "from:", "subject:", "is:", "has:", "in:", "account:", "newer:",
    ] {
        assert!(screen.contains(operator), "{operator}\n{screen}");
    }

    t.send_str("fr").unwrap();
    t.wait_until(|s| hint_line(s).contains("/fr") && !s.contains("subject:"))
        .unwrap();
    t.send(Key::Tab).unwrap();
    t.wait_until(|s| hint_line(s).contains("/from:")).unwrap();

    // Correspondents by a word of their name.
    t.send_str("ada").unwrap();
    t.wait_until(|s| s.contains("ada@lovelace.org")).unwrap();
    t.send(Key::Tab).unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("/from:ada@lovelace.org"))
        .unwrap();
    assert_eq!(selected_subject(&screen).as_deref(), Some("+ Engine notes"));

    // Tab and Shift-Tab go round the suggestions.
    t.send_str(" is:").unwrap();
    t.wait_until(|s| s.contains("not yet read")).unwrap();
    t.send(Key::Tab).unwrap();
    t.wait_until(|s| ends(s, "is:unread")).unwrap();
    t.send(Key::Tab).unwrap();
    t.wait_until(|s| ends(s, "is:read")).unwrap();
    t.send(Key::BackTab).unwrap();
    t.wait_until(|s| ends(s, "is:unread")).unwrap();
    quit(t);
}

#[test]
fn a_folder_with_a_space_is_written_quoted() {
    let server = FakeImap::start(people());
    server.add_folder(
        "Project X",
        &[],
        vec![message(
            "Linus",
            "Kernel plan",
            "2024-05-01T09:00:00Z",
            "Merge window",
        )],
    );
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Bernoulli")).unwrap();
    show(&mut t, "proj", " Project X · ", "Kernel plan");
    show(&mut t, "inbox", " Inbox · ", "Engine notes");

    t.send(Key::Char('/')).unwrap();
    t.send_str("in:proj").unwrap();
    t.wait_until(|s| s.contains("Project X")).unwrap();
    t.send(Key::Tab).unwrap();
    let screen = t
        .snapshot_after(|s| {
            hint_line(s).contains("/in:\"Project X\"") && list(s).contains("Kernel plan")
        })
        .unwrap();
    assert!(!list(&screen).contains("Engine notes"), "{screen}");

    // A space inside an open quote is still the word.
    for _ in 0.."in:\"Project X\"".len() {
        t.send(Key::Backspace).unwrap();
    }

    t.send_str("in:\"project ").unwrap();
    t.wait_until(|s| hint_line(s).contains("/in:\"project") && s.contains("Project X"))
        .unwrap();
    quit(t);
}

#[test]
fn escape_closes_the_menu_before_it_ends_the_search() {
    let server = FakeImap::start(people());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Bernoulli")).unwrap();

    t.send(Key::Char('/')).unwrap();
    t.send_str("i").unwrap();
    t.wait_until(|s| s.contains("unread or read")).unwrap();
    t.send(Key::Esc).unwrap();
    let screen = t.snapshot_after(|s| !s.contains("unread or read")).unwrap();
    assert!(hint_line(&screen).contains("/i"), "{screen}");

    // Still typing: the next key goes into the Query.
    t.send_str("nvoice").unwrap();
    t.wait_until(|s| hint_line(s).contains("/invoice")).unwrap();
    t.send(Key::Esc).unwrap();
    let screen = t
        .snapshot_after(|s| !hint_line(s).contains("/invoice"))
        .unwrap();
    assert!(list(&screen).contains("Engine notes"), "{screen}");
    quit(t);
}

#[test]
fn a_message_read_while_the_search_is_open_stays_in_it() {
    let server = FakeImap::start(people());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Bernoulli")).unwrap();

    // Selected, the match is marked read after a Dwell: dropping it would
    // select the next Unread, and so read them all.
    t.send(Key::Char('/')).unwrap();
    t.send_str("is:unread").unwrap();
    wait_selected(&mut t, "+ Engine notes");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !server.seen(INBOX, FIRST_UID) {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }

    // Shown read, or gone.
    let unread = |s: &Screen| {
        let row = (0..s.rows()).find(|&row| s.row_text(row).contains("Ada"));
        row.and_then(|row| s.cell(row, 2))
            .map(|cell| cell.contents() == "●")
    };
    let screen = t.snapshot_after(|s| unread(s) != Some(true)).unwrap();
    assert!(list(&screen).contains("Engine notes"), "{screen}");
    assert!(!list(&screen).contains("Compiler draft"), "{screen}");
    quit(t);
}

#[test]
fn tab_enters_the_menu_where_j_and_k_go_round_it() {
    let server = FakeImap::start(people());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Bernoulli")).unwrap();

    // Straight from `/`: the menu of every operator.
    t.send(Key::Char('/')).unwrap();
    t.wait_until(|s| s.contains("by name or address")).unwrap();
    t.send(Key::Tab).unwrap();
    t.wait_until(|s| ends(s, "/from:")).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| ends(s, "/to:")).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| ends(s, "/cc:")).unwrap();
    t.send(Key::Char('k')).unwrap();
    t.send(Key::Char('k')).unwrap();
    t.wait_until(|s| ends(s, "/from:")).unwrap();

    // Enter takes it, and the menu suggests for it.
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("ada@lovelace.org"))
        .unwrap();
    assert!(ends(&screen, "/from:"), "{screen}");

    // Out of the menu, `j` is typed.
    t.send_str("j").unwrap();
    t.wait_until(|s| ends(s, "/from:j")).unwrap();
    quit(t);
}
