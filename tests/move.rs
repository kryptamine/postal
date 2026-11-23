//! Move with `m` through the Folder Picker and Archive with `e`, through
//! the fake server's `UID MOVE`, and their Undo with `u`.

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
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, INBOX, Outage},
    home::Home,
    picker::{move_rows, mover, picked, picker},
};
use termlens::{Key, Screen, Terminal};

const ARCHIVE: &str = "Archive";
const ALL_MAIL: &str = "[Gmail]/All Mail";

/// The Message List's columns beside the Preview at 120 columns.
const LIST: usize = 42;

fn postal(home: &Home) -> Terminal {
    Terminal::builder()
        .size(120, 30)
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
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[track_caller]
fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "timed out at {}",
            std::panic::Location::caller()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn message(sender: &'static str, subject: &str, date: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(&format!("Body of {subject}")))
}

/// An Inbox of three Threads, newest first: Carol's, Bob's and Alice's,
/// beside an empty Work Folder.
fn server() -> FakeImap {
    let server = FakeImap::start(vec![
        message("Alice", "Oldest", "2024-03-01T09:00:00Z"),
        message("Bob", "Middle", "2024-03-02T09:00:00Z"),
        message("Carol", "Newest", "2024-03-03T09:00:00Z"),
    ]);
    server.add_folder("Work", &[], vec![]);
    server
}

/// Started on the newest Message's Preview, with the sync idling.
fn ready(server: &FakeImap, home: &Home) -> Terminal {
    let mut t = postal(home);
    t.wait_until(|s| s.contains("Body of Newest")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    t
}

/// The Message List's rows, top to bottom.
fn list(screen: &Screen) -> String {
    (0..screen.rows())
        .map(|row| screen.row_text(row).chars().take(LIST).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn hint_line(screen: &Screen) -> String {
    screen.row_text(screen.rows() - 1)
}

/// The `UID MOVE` and `UID COPY` commands sent, in order.
fn moves(server: &FakeImap) -> Vec<String> {
    server
        .commands()
        .into_iter()
        .filter(|c| c.starts_with("UID MOVE") || c.starts_with("UID COPY"))
        .collect()
}

/// Moves the selected Thread to the Folder the picker finds for `query`.
fn move_to(t: &mut Terminal, query: &str) {
    t.send(Key::Char('m')).unwrap();
    t.wait_until(|s| !mover(s).is_empty()).unwrap();
    t.send_str(query).unwrap();
    t.snapshot_after(|s| {
        mover(s).first().is_some_and(|q| q.ends_with(query)) && !move_rows(s).is_empty()
    })
    .unwrap();
    t.send(Key::Enter).unwrap();
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

#[test]
fn m_moves_the_selected_thread_to_the_folder_picked() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('m')).unwrap();
    let screen = t
        .snapshot_after(|s| move_rows(s).iter().any(|r| r.contains("Work")))
        .unwrap();
    // The Folder the Messages are in is no place to move them.
    assert!(
        !move_rows(&screen).iter().any(|r| r.contains("Inbox")),
        "{screen}"
    );
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| mover(s).is_empty()).unwrap();

    move_to(&mut t, "work");
    let screen = t
        .snapshot_after(|s| !list(s).contains("Newest") && s.contains("Body of Middle"))
        .unwrap();
    assert!(
        hint_line(&screen).contains("moved to Work · u to undo"),
        "{screen}"
    );
    wait_for(|| server.subjects("Work") == ["Newest"]);
    quit(t);
    assert_eq!(server.subjects(INBOX), ["Oldest", "Middle"]);
    assert_eq!(
        moves(&server),
        [format!("UID MOVE {} \"Work\"", FIRST_UID + 2)]
    );
}

#[test]
fn u_moves_a_moved_thread_back() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    move_to(&mut t, "work");
    wait_for(|| server.subjects("Work") == ["Newest"]);

    t.send(Key::Char('u')).unwrap();
    let screen = t
        .snapshot_after(|s| list(s).contains("Newest") && s.contains("Body of Newest"))
        .unwrap();
    assert!(hint_line(&screen).contains("restored"), "{screen}");
    wait_for(|| server.subjects("Work").is_empty() && server.subjects(INBOX).len() == 3);
    quit(t);
}

#[test]
fn e_archives_to_the_folder_with_the_archive_role() {
    let server = server();
    server.without_gmail();
    server.add_folder(ARCHIVE, &["\\Archive"], vec![]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('e')).unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Newest") && s.contains("Body of Middle"))
        .unwrap();
    assert!(
        hint_line(&screen).contains("archived · u to undo"),
        "{screen}"
    );
    wait_for(|| server.subjects(ARCHIVE) == ["Newest"]);

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| list(s).contains("Newest")).unwrap();
    wait_for(|| server.subjects(ARCHIVE).is_empty() && server.subjects(INBOX).len() == 3);
    quit(t);
}

// Gmail has no Archive Folder: archiving moves to All Mail, which takes the
// Inbox label off, and the Undo copies it back, adding the label again.
#[test]
fn on_gmail_archive_takes_the_inbox_label_off_and_undo_puts_it_back() {
    let server = server();
    server.add_all_mail(ALL_MAIL);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('e')).unwrap();
    t.wait_until(|s| !list(s).contains("Newest") && hint_line(s).contains("archived"))
        .unwrap();
    wait_for(|| server.subjects(INBOX) == ["Oldest", "Middle"]);
    assert_eq!(server.subjects(ALL_MAIL), ["Oldest", "Middle", "Newest"]);

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| list(s).contains("Newest")).unwrap();
    wait_for(|| server.subjects(INBOX).len() == 3);
    quit(t);
    assert_eq!(server.subjects(ALL_MAIL), ["Oldest", "Middle", "Newest"]);
    let moves = moves(&server);
    assert_eq!(
        moves[0],
        format!("UID MOVE {} \"{ALL_MAIL}\"", FIRST_UID + 2)
    );
    assert_eq!(moves[1], format!("UID COPY {} \"INBOX\"", FIRST_UID + 2));
}

#[test]
fn archive_in_the_archive_does_nothing() {
    let server = server();
    server.add_folder(
        ARCHIVE,
        &["\\Archive"],
        vec![message("Eve", "Filed", "2024-01-01T09:00:00Z")],
    );
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    show(&mut t, "archive", " Archive · ", "Body of Filed");

    t.send(Key::Char('e')).unwrap();
    t.wait_until(|s| hint_line(s).contains("already archived"))
        .unwrap();
    quit(t);
    assert_eq!(moves(&server), Vec::<String>::new());
}

#[test]
fn without_an_archive_folder_the_archive_is_taken_back() {
    let server = server();
    server.without_gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('e')).unwrap();
    let screen = t
        .snapshot_after(|s| {
            hint_line(s).contains("no Archive Folder") && list(s).contains("Newest")
        })
        .unwrap();
    quit(t);
    assert!(moves(&server).is_empty(), "{screen}");
}

// A move made offline waits in the Cache, across a restart.
#[test]
fn a_move_made_offline_reaches_the_server_on_the_next_run() {
    let server = server();
    let home = Home::for_server(&server);
    // The first run lists the Folders into the Cache, for the picker.
    let mut t = ready(&server, &home);
    t.send(Key::Char('m')).unwrap();
    t.wait_until(|s| move_rows(s).iter().any(|r| r.contains("Work")))
        .unwrap();
    quit(t);
    server.fail(Outage::Offline);

    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Body of Newest")).unwrap();
    move_to(&mut t, "work");
    t.wait_until(|s| !list(s).contains("Newest")).unwrap();
    quit(t);

    server.recover();
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Body of Middle")).unwrap();
    wait_for(|| server.subjects("Work") == ["Newest"]);
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    let screen = t.snapshot_after(|_| true).unwrap();
    quit(t);
    assert!(!list(&screen).contains("Newest"), "{screen}");
}

// All Mail never loses what an Archive's Undo copies back to the Inbox.
#[test]
fn after_a_gmail_archive_is_undone_all_mail_still_lists_the_message() {
    let server = server();
    server.add_all_mail(ALL_MAIL);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    show(&mut t, "all mail", " All Mail · ", "Body of Newest");
    show(&mut t, "inbox", " Inbox · ", "Body of Newest");

    t.send(Key::Char('e')).unwrap();
    t.wait_until(|s| !list(s).contains("Newest")).unwrap();
    wait_for(|| server.subjects(INBOX).len() == 2);
    t.send(Key::Char('u')).unwrap();
    wait_for(|| server.subjects(INBOX).len() == 3);
    show(&mut t, "all mail", " All Mail · ", "Body of Newest");
    let screen = t.snapshot_after(|s| list(s).contains("Newest")).unwrap();
    quit(t);
    assert!(list(&screen).contains("Middle"), "{screen}");
}

// On Gmail a move between labels is a `UID MOVE` all the same, and its Undo
// moves it back by the UID the label gave it.
#[test]
fn on_gmail_a_move_to_a_label_and_its_undo_are_moves() {
    let server = server();
    server.add_all_mail(ALL_MAIL);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    move_to(&mut t, "work");
    wait_for(|| server.subjects("Work") == ["Newest"]);

    t.send(Key::Char('u')).unwrap();
    wait_for(|| server.subjects(INBOX).len() == 3 && server.subjects("Work").is_empty());
    quit(t);
    assert_eq!(server.subjects(ALL_MAIL), ["Oldest", "Middle", "Newest"]);
    let moves = moves(&server);
    assert_eq!(moves[0], format!("UID MOVE {} \"Work\"", FIRST_UID + 2));
    assert!(
        moves[1].starts_with("UID MOVE ") && moves[1].ends_with(" \"INBOX\""),
        "{moves:#?}"
    );
}

// Without SPECIAL-USE, as on iCloud, the Folder named Archive is the one.
#[test]
fn without_special_use_archive_goes_to_the_folder_named_archive() {
    let server = server();
    server.without_gmail();
    server.add_folder(ARCHIVE, &[], vec![]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('e')).unwrap();
    wait_for(|| server.subjects(ARCHIVE) == ["Newest"]);
    quit(t);
    assert_eq!(server.subjects(INBOX), ["Oldest", "Middle"]);
}

// On Gmail a Delete from All Mail is a move to Trash, which All Mail
// never stops a Message from taking.
#[test]
fn on_gmail_a_delete_in_all_mail_moves_to_trash() {
    let server = server();
    server.add_all_mail(ALL_MAIL);
    server.add_folder("[Gmail]/Trash", &["\\Trash"], vec![]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    show(&mut t, "all mail", " All Mail · ", "Body of Newest");

    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !list(s).contains("Newest")).unwrap();
    wait_for(|| server.subjects("[Gmail]/Trash") == ["Newest"]);
    quit(t);
    assert_eq!(server.subjects(INBOX), ["Oldest", "Middle"]);
}
