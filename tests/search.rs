//! Search: `/` narrows the Message List to the Messages of every cached
//! Folder with words starting with the Query's, best match first, typed in
//! the hint line.

use crate::support;

use support::{
    fake_imap::{EMAIL, FakeImap},
    home::Home,
    search::{
        above, hint_line, list, message, messages, quit, ready, selected_subject, show,
        wait_selected,
    },
};
use termlens::{Color, Key, Terminal};

#[test]
fn a_query_narrows_the_list_best_match_first_and_the_preview_follows() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("pay").unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Holiday") && s.contains("Please pay by Friday"))
        .unwrap();
    // `pay` after a space in Alice's Snippet beats Carol's after a hyphen,
    // though Carol's Message is newer.
    assert!(
        above(&screen, "Invoice 1042", "Quarterly report"),
        "{screen}"
    );
    for gone in ["Lunch", "Weekly digest"] {
        assert!(!list(&screen).contains(gone), "{gone}: {screen}");
    }

    assert_eq!(selected_subject(&screen).as_deref(), Some("Invoice 1042"));
    assert!(
        hint_line(&screen).trim_start().starts_with("/pay"),
        "{screen}"
    );

    // `^j` arrives as `\n`, which a raw terminal must not read as Enter.
    t.send(Key::Ctrl('j')).unwrap();
    let screen = wait_selected(&mut t, "Quarterly report");
    assert!(screen.contains("Pages and a yearly summary"), "{screen}");
    let row = (0..screen.rows())
        .find(|&row| screen.row_text(row).contains("Please pay by"))
        .unwrap();
    let text = screen.row_text(row);
    let col = u16::try_from(text.split_once("pay by").unwrap().0.chars().count()).unwrap();
    for offset in 0..3 {
        let style = screen.cell(row, col + offset).unwrap().style();
        assert!(
            style.bold && style.fg == Color::Indexed(2),
            "{offset}: {style:?}"
        );
    }

    t.send(Key::Ctrl('k')).unwrap();
    wait_selected(&mut t, "Invoice 1042");

    // Enter keeps the Query; the list keys work on the matches.
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("search"))
        .unwrap();
    assert!(hint_line(&screen).contains("/pay"), "{screen}");
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Quarterly report");
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Quarterly report");

    // Esc ends the Search on the Message selected in it.
    t.send(Key::Esc).unwrap();
    let screen = t
        .snapshot_after(|s| list(s).contains("Holiday photos"))
        .unwrap();
    assert_eq!(
        selected_subject(&screen).as_deref(),
        Some("Quarterly report")
    );
    assert!(!hint_line(&screen).contains("/pay"), "{screen}");
    quit(t);
}

#[test]
fn escape_while_typing_restores_the_selection_held_before_the_search() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Lunch");

    t.send(Key::Char('/')).unwrap();
    t.send_str("pay").unwrap();
    wait_selected(&mut t, "Invoice 1042");
    t.send(Key::Esc).unwrap();
    let screen = wait_selected(&mut t, "Lunch");
    assert!(list(&screen).contains("Holiday photos"), "{screen}");
    assert!(screen.contains("The invoice is attached"), "{screen}");

    // `/` on a kept Query edits it; Esc then comes back to that `/`.
    t.send(Key::Char('/')).unwrap();
    t.send_str("pay").unwrap();
    t.send(Key::Enter).unwrap();
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Quarterly report");
    t.send(Key::Char('/')).unwrap();
    t.send(Key::Char('s')).unwrap();
    t.wait_until(|s| hint_line(s).contains("/pays")).unwrap();
    t.send(Key::Esc).unwrap();
    let screen = t
        .snapshot_after(|s| list(s).contains("Holiday photos"))
        .unwrap();
    assert_eq!(
        selected_subject(&screen).as_deref(),
        Some("Quarterly report")
    );
    quit(t);
}

#[test]
fn a_query_matching_nothing_says_so_and_leaves_the_preview_empty() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("zqx").unwrap();
    let screen = t.snapshot_after(|s| s.contains("no matches")).unwrap();
    assert!(!screen.contains("From "), "{screen}");
    assert!(!screen.contains("Loading"), "{screen}");

    // An empty Query is the whole list again, newest first.
    for _ in 0..3 {
        t.send(Key::Backspace).unwrap();
    }

    let screen = wait_selected(&mut t, "Quarterly report");
    assert!(
        above(&screen, "Quarterly report", "Invoice 1042"),
        "{screen}"
    );
    assert!(above(&screen, "Invoice 1042", "Lunch"), "{screen}");
    quit(t);
}

#[test]
fn an_uppercase_letter_makes_the_query_case_sensitive() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("inv").unwrap();
    // The unfiltered list has Lunch and Invoice too: Holiday gone is the query.
    t.wait_until(|s| {
        let list = list(s);
        list.contains("Lunch") && list.contains("Invoice") && !list.contains("Holiday")
    })
    .unwrap();

    t.send(Key::Backspace).unwrap();
    t.send(Key::Backspace).unwrap();
    t.send(Key::Backspace).unwrap();
    t.send_str("Inv").unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Lunch") && list(s).contains("Invoice"))
        .unwrap();
    assert!(!list(&screen).contains("Holiday"), "{screen}");
    quit(t);
}

#[test]
fn a_tie_goes_to_the_newer_message() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("digest").unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Lunch") && list(s).contains("Edition 1"))
        .unwrap();
    assert!(above(&screen, "Edition 2", "Edition 1"), "{screen}");
    quit(t);
}

#[test]
fn a_new_message_that_matches_joins_the_results_and_the_selection_stays() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("pay").unwrap();
    wait_selected(&mut t, "Invoice 1042");
    t.send(Key::Ctrl('j')).unwrap();
    wait_selected(&mut t, "Quarterly report");

    server.deliver(message("Frank", "Hello", "2024-06-02T00:00:00Z", "Hi"));
    server.deliver(message(
        "Erin",
        "Payslip",
        "2024-06-01T00:00:00Z",
        "For May",
    ));
    let screen = t.snapshot_after(|s| list(s).contains("Payslip")).unwrap();
    assert!(!list(&screen).contains("Hello"), "{screen}");
    assert_eq!(
        selected_subject(&screen).as_deref(),
        Some("Quarterly report")
    );
    // Still typing: the Query takes the next character.
    t.send(Key::Char('s')).unwrap();
    let screen = t.snapshot_after(|s| !list(s).contains("Invoice")).unwrap();
    assert!(hint_line(&screen).contains("/pays"), "{screen}");
    assert_eq!(selected_subject(&screen).as_deref(), Some("Payslip"));
    quit(t);
}

/// Gmail, with each Folder opened once so the Cache holds it: Sent holds a
/// booking too far down to be fetched, Trash a refund, and All Mail a copy
/// of every Message but the refund.
fn gmail(server: &FakeImap, home: &Home) -> Terminal {
    server.add_folder(
        "[Gmail]/Sent Mail",
        &["\\Sent"],
        vec![
            message("Me", "Flight booking", "2024-01-02T09:00:00Z", "Seat 14A"),
            message("Me", "Dentist", "2024-01-03T09:00:00Z", "Tuesday"),
            message("Me", "Gym", "2024-01-05T09:00:00Z", "Mondays"),
            message("Me", "Rent", "2024-01-06T09:00:00Z", "Paid"),
        ],
    );
    server.add_folder(
        "[Gmail]/Trash",
        &["\\Trash"],
        vec![message(
            "Eve",
            "Flight refund",
            "2024-01-04T09:00:00Z",
            "Sorry",
        )],
    );
    server.add_all_mail("[Gmail]/All Mail");
    let mut t = ready(server, home);
    show(&mut t, "sent", " Sent · ", "Rent");
    show(&mut t, "trash", " Trash · ", "Flight refund");
    show(&mut t, "all", " All Mail · ", "Flight booking");
    show(&mut t, "inbox", " Inbox · ", "Holiday photos");
    t
}

fn body_cached(home: &Home, subject: &str) -> bool {
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    cache
        .query_row(
            "SELECT count(*) FROM body JOIN message ON message.body_key = body.key
             WHERE message.subject = ?1",
            [subject],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
        > 0
}

#[test]
fn a_search_finds_messages_in_every_cached_folder_but_trash() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = gmail(&server, &home);

    assert!(!body_cached(&home, "Flight booking"));
    t.send(Key::Char('/')).unwrap();
    t.send_str("flight").unwrap();
    // Its body is asked of Sent, not of the shown Inbox; the Snippet in the
    // list can show the text before the cursor reaches it.
    let screen = t
        .snapshot_after(|s| {
            s.contains("Seat 14A") && selected_subject(s).as_deref() == Some("Flight booking")
        })
        .unwrap();
    assert!(!list(&screen).contains("Flight refund"), "{screen}");
    assert_eq!(
        list(&screen).matches("Flight booking").count(),
        1,
        "{screen}"
    );

    // A Message in the Inbox and All Mail is one match.
    for _ in 0.."flight".len() {
        t.send(Key::Backspace).unwrap();
    }

    t.send_str("1042").unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Holiday") && list(s).contains("Invoice 1042"))
        .unwrap();
    assert_eq!(list(&screen).matches("Invoice 1042").count(), 1, "{screen}");
    quit(t);
}

#[test]
fn ending_a_search_on_a_match_elsewhere_stays_in_the_shown_folder() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = gmail(&server, &home);
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('j')).unwrap();
    wait_selected(&mut t, "Lunch");

    t.send(Key::Char('/')).unwrap();
    t.send_str("seat").unwrap();
    wait_selected(&mut t, "Flight booking");
    t.send(Key::Enter).unwrap();
    t.send(Key::Esc).unwrap();
    // The Inbox lacks the match's Thread: back where `/` was pressed.
    let screen = t
        .snapshot_after(|s| list(s).contains("Holiday photos"))
        .unwrap();
    assert!(screen.row_text(0).contains(" Inbox · "), "{screen}");
    assert_eq!(selected_subject(&screen).as_deref(), Some("Lunch"));
    quit(t);
}

#[test]
fn a_query_finds_a_sent_message_by_its_recipients_and_no_other() {
    let mut inbox = messages();
    inbox.push(
        message("Me", "Contract", "2024-02-01T09:00:00Z", "Signed copy")
            .sent_by_me()
            // Петр Новак, as an encoded word.
            .to(
                "=?UTF-8?B?0J/QtdGC0YAg0J3QvtCy0LDQug==?=",
                "p.novak@example.org",
            )
            .cc("Olga Smirnova", "olga@example.org"),
    );
    // A recipient of mail someone else sent is no way to find it.
    inbox.push(
        message("Erin", "Agenda", "2024-02-02T09:00:00Z", "For Monday")
            .to("Petr Novak", "p.novak@example.org"),
    );
    let server = FakeImap::start(inbox);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    for query in ["петр", "p.novak", "smirnova"] {
        t.send_str(query).unwrap();
        let screen = t
            .snapshot_after(|s| !list(s).contains("Holiday") && hint_line(s).contains(query))
            .unwrap();
        assert_eq!(selected_subject(&screen).as_deref(), Some("Contract"));
        assert!(!list(&screen).contains("Agenda"), "{screen}");
        for _ in query.chars() {
            t.send(Key::Backspace).unwrap();
        }

        // Carol's matches none: the list is whole again.
        t.wait_until(|s| list(s).contains("Quarterly")).unwrap();
    }

    quit(t);
}

#[test]
fn a_query_matching_only_inside_a_word_finds_nothing() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    // The price of a word index: `voice` no longer finds the invoices.
    t.send(Key::Char('/')).unwrap();
    t.send_str("voice").unwrap();
    let screen = t.snapshot_after(|s| s.contains("no matches")).unwrap();
    assert!(!list(&screen).contains("Invoice"), "{screen}");
    quit(t);
}

#[test]
fn a_message_deleted_while_a_search_is_open_leaves_the_results() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('/')).unwrap();
    t.send_str("pay").unwrap();
    // A match found later in the bodies joins the results and can move the
    // cursor's row: delete only once both are in.
    t.wait_until(|s| list(s).contains("Quarterly report"))
        .unwrap();
    wait_selected(&mut t, "Invoice 1042");
    t.send(Key::Enter).unwrap();
    t.send(Key::Backspace).unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Invoice 1042"))
        .unwrap();
    assert_eq!(
        selected_subject(&screen).as_deref(),
        Some("Quarterly report")
    );
    assert!(hint_line(&screen).contains("/pay"), "{screen}");
    quit(t);
}

/// A Search reads this many candidates at most.
const LIMIT: u32 = 500;

#[test]
fn a_query_matching_more_than_a_search_reads_says_so() {
    let server = FakeImap::start(messages());
    server.add_folder("Lists", &[], vec![]);
    let home = Home::for_server(&server);
    quit(ready(&server, &home));

    // Written straight into the Cache: no sync caches this many in a test.
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    cache
        .execute(
            "INSERT OR IGNORE INTO mailbox (account, name) VALUES (?1, 'Lists')",
            [EMAIL.to_lowercase()],
        )
        .unwrap();
    for uid in 1..=LIMIT {
        cache
            .execute(
                "INSERT INTO message (account, mailbox, uid, body_key, received,
                   received_offset, unread, answered, sender, from_line, to_line, subject,
                   from_me, snippet, attachments)
                 VALUES (?1, 'Lists', ?2, ?2, 1600000000 + ?2, 0, 0, 0, 'List', '', '',
                   'Daily digest', 0, '', '')",
                rusqlite::params![EMAIL.to_lowercase(), uid],
            )
            .unwrap();
    }

    drop(cache);
    let mut t = ready(&server, &home);
    t.send(Key::Char('/')).unwrap();
    t.send_str("digest").unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("type more"))
        .unwrap();
    assert!(hint_line(&screen).contains("newest 500"), "{screen}");

    for _ in 0.."digest".len() {
        t.send(Key::Backspace).unwrap();
    }

    t.send_str("weekly").unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("/weekly") && list(s).contains("Edition 2"))
        .unwrap();
    assert!(!hint_line(&screen).contains("type more"), "{screen}");
    quit(t);
}
