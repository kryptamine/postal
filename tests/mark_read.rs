//! Mark Read after a Dwell, at once on entering Reading, and Mark Unread
//! with `U`, through the fake server's `UID STORE`.

use crate::support;

use std::time::Duration;

use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, INBOX, Outage, Stall},
    home::Home,
    picker::{picked, picker},
    quit, wait_for,
};
use termlens::{Key, Screen, Terminal};

const READ: &str = "+FLAGS.SILENT (\\Seen)";
const UNREAD: &str = "-FLAGS.SILENT (\\Seen)";

fn postal(home: &Home) -> Terminal {
    home.terminal()
        .size(120, 30)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn message(sender: &'static str, subject: &str, date: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(&format!("Body of {subject}")))
}

/// The `UID STORE` commands sent, in order.
fn stores(server: &FakeImap) -> Vec<String> {
    server
        .commands()
        .into_iter()
        .filter(|c| c.starts_with("UID STORE"))
        .collect()
}

/// Whether the card naming `sender` shows the Unread marker; `None` when
/// no card on screen names it.
fn card(screen: &Screen, sender: &str) -> Option<bool> {
    let row = (0..screen.rows()).find(|&row| screen.row_text(row).contains(sender))?;
    Some(screen.cell(row, 2)?.contents() == "●")
}

/// Started on the newest Message's Preview, with the sync idling.
fn ready(server: &FakeImap, home: &Home, newest: &str) -> Terminal {
    let mut t = postal(home);
    t.wait_until(|s| s.contains(&format!("Body of {newest}")))
        .unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    t
}

#[test]
fn a_dwell_on_an_unread_thread_marks_it_read() {
    let server = FakeImap::start(vec![
        message("Alice", "Older", "2024-03-01T09:00:00Z"),
        message("Carol", "Newest", "2024-03-02T09:00:00Z").unread(),
    ]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");

    let screen = t
        .snapshot_after(|s| card(s, "Carol") == Some(false))
        .unwrap();
    wait_for(|| server.seen(INBOX, FIRST_UID + 1));
    quit(t);
    assert_eq!(
        stores(&server),
        [format!("UID STORE {} {READ}", FIRST_UID + 1)],
        "{screen}"
    );
}

// Another client's change the server sends with the answer to a Mark Read
// was dropped, and IDLE never sent it again: the row kept its old flag.
#[test]
fn a_flag_change_sent_with_a_stores_answer_is_taken_in() {
    let server = FakeImap::start(vec![
        message("Alice", "Older", "2024-03-01T09:00:00Z"),
        message("Carol", "Newest", "2024-03-02T09:00:00Z").unread(),
    ]);
    let home = Home::for_server(&server);
    server.stall(Stall::Store);
    let mut t = ready(&server, &home, "Newest");
    wait_for(|| !stores(&server).is_empty());
    server.set_seen(FIRST_UID, false);
    server.release();

    t.wait_until(|s| card(s, "Alice") == Some(true)).unwrap();
    quit(t);
}

// Moving through the list must not read every row it passes.
#[test]
fn scrolling_past_an_unread_thread_leaves_it_unread() {
    let server = FakeImap::start(vec![
        message("Alice", "Oldest", "2024-03-01T09:00:00Z").unread(),
        message("Bob", "Passed", "2024-03-02T09:00:00Z").unread(),
        message("Carol", "Newest", "2024-03-03T09:00:00Z"),
    ]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("Body of Oldest")).unwrap();

    // A Dwell begun on the row passed would have ended first.
    wait_for(|| server.seen(INBOX, FIRST_UID));
    let screen = t.snapshot_after(|_| true).unwrap();
    quit(t);
    assert_eq!(card(&screen, "Bob"), Some(true), "{screen}");
    assert_eq!(
        stores(&server),
        [format!("UID STORE {FIRST_UID} {READ}")],
        "{screen}"
    );
    assert!(!server.seen(INBOX, FIRST_UID + 1));
}

#[test]
fn entering_reading_marks_every_unread_message_of_the_thread_read() {
    let server = FakeImap::start(vec![
        message("Alice", "Plans", "2024-03-01T09:00:00Z")
            .in_thread(0)
            .unread(),
        message("Bob", "Re: Plans", "2024-03-02T09:00:00Z")
            .in_thread(0)
            .unread(),
        message("Carol", "Elsewhere", "2024-03-01T08:00:00Z").unread(),
    ]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Re: Plans");
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.row_text(0).contains(" 1/2 ")).unwrap();

    wait_for(|| server.seen(INBOX, FIRST_UID) && server.seen(INBOX, FIRST_UID + 1));
    quit(t);
    assert_eq!(
        stores(&server),
        [format!("UID STORE {},{} {READ}", FIRST_UID, FIRST_UID + 1)]
    );
    assert!(!server.seen(INBOX, FIRST_UID + 2));
}

// `U` undoes a Dwell's Mark Read, and the Dwell must not take it back
// until the selection leaves the Thread and comes back.
#[test]
fn mark_unread_holds_until_the_selection_comes_back() {
    let server = FakeImap::start(vec![
        message("Alice", "Older", "2024-03-01T09:00:00Z"),
        message("Carol", "Newest", "2024-03-02T09:00:00Z").unread(),
    ]);
    let home = Home::for_server(&server);
    let t = home.terminal().size(120, 30).env("POSTAL_DWELL", "0.2");
    let mut t = t.spawn(env!("CARGO_BIN_EXE_postal")).unwrap();
    t.wait_until(|s| s.contains("Body of Newest")).unwrap();
    wait_for(|| server.seen(INBOX, FIRST_UID + 1));

    t.send(Key::Char('U')).unwrap();
    let screen = t
        .snapshot_after(|s| card(s, "Carol") == Some(true) && s.contains("marked Unread"))
        .unwrap();
    wait_for(|| !server.seen(INBOX, FIRST_UID + 1));
    // Twice the Dwell set above.
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(stores(&server).len(), 2, "{screen}");
    assert!(!server.seen(INBOX, FIRST_UID + 1));

    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("Body of Older")).unwrap();
    t.send(Key::Char('k')).unwrap();
    t.wait_until(|s| card(s, "Carol") == Some(false)).unwrap();
    wait_for(|| server.seen(INBOX, FIRST_UID + 1));
    quit(t);
    let uid = FIRST_UID + 1;
    assert_eq!(
        stores(&server),
        [
            format!("UID STORE {uid} {READ}"),
            format!("UID STORE {uid} {UNREAD}"),
            format!("UID STORE {uid} {READ}"),
        ]
    );
}

#[test]
fn a_search_match_in_another_folder_is_marked_there() {
    let server = FakeImap::start(vec![message("Alice", "Hello", "2024-03-01T09:00:00Z")]);
    server.add_folder(
        "Work",
        &[],
        vec![
            message("Dana", "Budget", "2024-02-01T09:00:00Z").unread(),
            message("Erin", "Standup", "2024-02-02T09:00:00Z"),
        ],
    );
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Hello");
    show(&mut t, "work", " Work · ", "Body of Standup");
    show(&mut t, "inbox", " Inbox · ", "Body of Hello");

    t.send(Key::Char('/')).unwrap();
    t.send_str("budget").unwrap();
    t.wait_until(|s| s.contains("Body of Budget")).unwrap();
    wait_for(|| server.seen("Work", FIRST_UID));
    quit(t);

    let commands = server.commands();
    let at = |wanted: &str| commands.iter().rposition(|c| c == wanted).unwrap();
    let store = at(&format!("UID STORE {FIRST_UID} {READ}"));
    assert!(at("SELECT \"Work\"") < store, "{commands:#?}");
    assert!(store < at("SELECT \"INBOX\""), "{commands:#?}");
}

// What the server announces of another Folder while a mark is stored there
// must not be read against the shown one, selected again after it.
#[test]
fn an_expunge_in_another_folder_leaves_the_shown_one_alone() {
    let server = FakeImap::start(vec![
        message("Alice", "Oldest", "2024-03-01T09:00:00Z"),
        message("Bob", "Middle", "2024-03-02T09:00:00Z"),
        message("Carol", "Unread", "2024-03-03T09:00:00Z").unread(),
        message("Dave", "Newest", "2024-03-04T09:00:00Z"),
    ]);
    // Ahead of INBOX by name, so its marks are stored first.
    server.add_folder(
        "Archive",
        &[],
        vec![
            message("Dana", "Budget", "2024-02-01T09:00:00Z").unread(),
            message("Erin", "Standup", "2024-02-02T09:00:00Z"),
        ],
    );
    server.add_folder("Trash", &["\\Trash"], Vec::new());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    show(&mut t, "archive", " Archive · ", "Body of Standup");
    show(&mut t, "inbox", " Inbox · ", "Body of Newest");

    // Both marks wait behind a held Delete, to go out in one pass.
    server.stall(Stall::Move);
    t.send(Key::Backspace).unwrap();
    wait_for(|| server.commands().iter().any(|c| c.starts_with("UID MOVE")));
    t.wait_until(|s| card(s, "Carol") == Some(false)).unwrap();
    t.send(Key::Char('/')).unwrap();
    t.send_str("budget").unwrap();
    t.wait_until(|s| card(s, "Dana") == Some(false)).unwrap();
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| s.contains("Body of Unread")).unwrap();

    server.clear_commands();
    server.stall(Stall::Store);
    wait_for(|| server.commands().iter().any(|c| c.starts_with("UID STORE")));
    server.empty("Archive");
    server.release();
    wait_for(|| server.seen(INBOX, FIRST_UID + 2));
    // Idling again once the shown Folder is read anew after the pass.
    wait_for(|| {
        let commands = server.commands();
        let last = |f: fn(&String) -> bool| commands.iter().rposition(f);
        last(|c| c == "IDLE") > last(|c| c.starts_with("UID STORE"))
    });
    let screen = t.snapshot_after(|_| true).unwrap();
    quit(t);
    assert!(
        screen.contains("Oldest") && screen.contains("Middle"),
        "{screen}"
    );
}

// A mark made offline waits in the Cache, across a restart, and the resync
// after it does not put the server's older flag back.
#[test]
fn a_mark_made_offline_reaches_the_server_on_the_next_run() {
    let server = FakeImap::start(vec![
        message("Alice", "Older", "2024-03-01T09:00:00Z"),
        message("Carol", "Newest", "2024-03-02T09:00:00Z"),
    ]);
    let home = Home::for_server(&server);
    quit(ready(&server, &home, "Newest"));
    server.set_seen(FIRST_UID + 1, false);
    server.fail(Outage::Offline);

    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Body of Newest")).unwrap();
    t.wait_until(|s| card(s, "Carol") == Some(false)).unwrap();
    quit(t);
    assert_eq!(stores(&server), Vec::<String>::new());

    server.recover();
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Body of Newest")).unwrap();
    wait_for(|| server.seen(INBOX, FIRST_UID + 1));
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    let screen = t.snapshot_after(|_| true).unwrap();
    quit(t);
    assert_eq!(card(&screen, "Carol"), Some(false), "{screen}");
}

/// Shows the Folder the picker finds for `query`, until its title reads
/// `title` and the Preview `body`.
fn show(t: &mut Terminal, query: &str, title: &str, body: &str) {
    t.send(Key::Char('f')).unwrap();
    t.wait_until(|s| !picker(s).is_empty()).unwrap();
    t.send_str(query).unwrap();
    t.snapshot_after(|s| {
        picker(s).first().is_some_and(|q| q.ends_with(query)) && !picked(s).is_empty()
    })
    .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.row_text(0).contains(title) && s.contains(body))
        .unwrap();
}
