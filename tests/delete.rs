//! Delete with `Backspace` and Undo with `u`, through the fake server's
//! `UID MOVE` to Trash and back.

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
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, INBOX, Outage, Stall},
    home::Home,
    picker::{picked, picker},
};
use termlens::{Key, Screen, Terminal};

const TRASH: &str = "[Gmail]/Trash";

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
/// beside an empty Trash.
fn server() -> FakeImap {
    let server = FakeImap::start(vec![
        message("Alice", "Oldest", "2024-03-01T09:00:00Z"),
        message("Bob", "Middle", "2024-03-02T09:00:00Z"),
        message("Carol", "Newest", "2024-03-03T09:00:00Z"),
    ]);
    server.add_folder(TRASH, &["\\Trash"], vec![]);
    server
}

/// Started on the newest Message's Preview, with the sync idling.
fn ready(server: &FakeImap, home: &Home, newest: &str) -> Terminal {
    let mut t = postal(home);
    t.wait_until(|s| s.contains(&format!("Body of {newest}")))
        .unwrap();
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

/// The `UID MOVE` commands sent, in order.
fn moves(server: &FakeImap) -> Vec<String> {
    server
        .commands()
        .into_iter()
        .filter(|c| c.starts_with("UID MOVE"))
        .collect()
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
fn backspace_moves_the_selected_thread_to_trash_and_selects_the_next() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");

    t.send(Key::Backspace).unwrap();
    let screen = t
        .snapshot_after(|s| !list(s).contains("Newest") && s.contains("Body of Middle"))
        .unwrap();
    assert!(
        hint_line(&screen).contains("deleted · u to undo"),
        "{screen}"
    );
    wait_for(|| server.subjects(TRASH) == ["Newest"]);
    quit(t);
    assert_eq!(server.subjects(INBOX), ["Oldest", "Middle"]);
    assert_eq!(
        moves(&server),
        [format!("UID MOVE {} \"{TRASH}\"", FIRST_UID + 2)]
    );
}

// Deleting the bottom row leaves the selection on the row above it.
#[test]
fn deleting_the_last_row_selects_the_one_above() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    t.send(Key::Char('G')).unwrap();
    t.wait_until(|s| s.contains("Body of Oldest")).unwrap();

    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !list(s).contains("Oldest") && s.contains("Body of Middle"))
        .unwrap();
    wait_for(|| server.subjects(TRASH) == ["Oldest"]);
    quit(t);
}

#[test]
fn delete_takes_every_message_of_the_thread() {
    let server = FakeImap::start(vec![
        message("Alice", "Plans", "2024-03-01T09:00:00Z").in_thread(0),
        message("Dana", "Elsewhere", "2024-03-01T10:00:00Z"),
        message("Bob", "Re: Plans", "2024-03-02T09:00:00Z").in_thread(0),
    ]);
    server.add_folder(TRASH, &["\\Trash"], vec![]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Re: Plans");

    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !list(s).contains("Plans") && s.contains("Body of Elsewhere"))
        .unwrap();
    wait_for(|| server.subjects(TRASH).len() == 2);
    quit(t);
    assert_eq!(server.subjects(INBOX), ["Elsewhere"]);
    assert_eq!(
        moves(&server),
        [format!(
            "UID MOVE {},{} \"{TRASH}\"",
            FIRST_UID,
            FIRST_UID + 2
        )]
    );
}

#[test]
fn in_reading_delete_takes_the_message_at_the_cursor() {
    let server = FakeImap::start(vec![
        message("Alice", "Plans", "2024-03-01T09:00:00Z").in_thread(0),
        message("Bob", "Re: Plans", "2024-03-02T09:00:00Z").in_thread(0),
        message("Dana", "Elsewhere", "2024-02-01T10:00:00Z"),
    ]);
    server.add_folder(TRASH, &["\\Trash"], vec![]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Re: Plans");
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.row_text(0).contains(" 2/2 ")).unwrap();

    t.send(Key::Backspace).unwrap();
    let screen = t
        .snapshot_after(|s| s.row_text(0).contains(" 1/1 ") && !s.contains("Body of Re: Plans"))
        .unwrap();
    assert!(screen.contains("Body of Plans"), "{screen}");
    wait_for(|| server.subjects(TRASH) == ["Re: Plans"]);

    // The Thread's last Message gone, the list is back.
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| s.contains("Body of Elsewhere") && !list(s).contains("Plans"))
        .unwrap();
    wait_for(|| server.subjects(TRASH).len() == 2);
    quit(t);
}

#[test]
fn u_undoes_the_latest_delete_and_then_the_one_before() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| s.contains("Body of Middle")).unwrap();
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| s.contains("Body of Oldest")).unwrap();
    wait_for(|| server.subjects(TRASH).len() == 2);

    t.send(Key::Char('u')).unwrap();
    let screen = t
        .snapshot_after(|s| list(s).contains("Middle") && s.contains("Body of Middle"))
        .unwrap();
    assert!(hint_line(&screen).contains("restored"), "{screen}");
    assert!(!list(&screen).contains("Newest"), "{screen}");
    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| list(s).contains("Newest") && s.contains("Body of Newest"))
        .unwrap();
    wait_for(|| server.subjects(TRASH).is_empty() && server.subjects(INBOX).len() == 3);

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| hint_line(s).contains("nothing to undo"))
        .unwrap();
    // The resync swaps in the UIDs the server gave the restored Messages.
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("Body of Middle")).unwrap();
    quit(t);
    let moves = moves(&server);
    let back = moves.iter().filter(|m| m.ends_with(" \"INBOX\"")).count();
    assert_eq!(back, 2, "{moves:#?}");
}

#[test]
fn an_undo_in_another_folder_restores_to_where_the_delete_was() {
    let server = server();
    server.add_folder(
        "Work",
        &[],
        vec![message("Erin", "Standup", "2024-02-02T09:00:00Z")],
    );
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| s.contains("Body of Middle")).unwrap();
    wait_for(|| server.subjects(TRASH) == ["Newest"]);
    show(&mut t, "work", " Work · ", "Body of Standup");

    t.send(Key::Char('u')).unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("restored to Inbox"))
        .unwrap();
    assert!(screen.row_text(0).contains(" Work · "), "{screen}");
    wait_for(|| server.subjects(INBOX).len() == 3);
    show(&mut t, "inbox", " Inbox · ", "Body of Middle");
    t.wait_until(|s| list(s).contains("Newest")).unwrap();
    quit(t);
}

#[test]
fn delete_in_trash_does_nothing() {
    let server = server();
    server.deliver_to(TRASH, message("Eve", "Junked", "2024-01-01T09:00:00Z"));
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    show(&mut t, "trash", " Trash · ", "Body of Junked");

    t.send(Key::Backspace).unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("already in Trash"))
        .unwrap();
    assert!(list(&screen).contains("Junked"), "{screen}");
    quit(t);
    assert!(moves(&server).is_empty());
}

#[test]
fn a_search_match_in_another_folder_is_deleted_from_there() {
    let server = server();
    server.add_folder(
        "Work",
        &[],
        vec![message("Erin", "Budget", "2024-02-02T09:00:00Z")],
    );
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    show(&mut t, "work", " Work · ", "Body of Budget");
    show(&mut t, "inbox", " Inbox · ", "Body of Newest");

    t.send(Key::Char('/')).unwrap();
    t.send_str("budget").unwrap();
    t.wait_until(|s| s.contains("Body of Budget")).unwrap();
    t.send(Key::Enter).unwrap();
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !list(s).contains("Budget")).unwrap();
    wait_for(|| server.subjects(TRASH) == ["Budget"]);
    quit(t);
    let commands = server.commands();
    let at = |wanted: &str| commands.iter().rposition(|c| c == wanted).unwrap();
    let moved = at(&format!("UID MOVE {FIRST_UID} \"{TRASH}\""));
    assert!(at("SELECT \"Work\"") < moved, "{commands:#?}");
    assert!(moved < at("SELECT \"INBOX\""), "{commands:#?}");
}

// Emptied elsewhere, Trash no longer has what the Undo would bring back.
#[test]
fn an_undo_after_trash_was_emptied_says_so() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    t.send(Key::Backspace).unwrap();
    wait_for(|| server.subjects(TRASH) == ["Newest"]);
    server.empty(TRASH);

    t.send(Key::Char('u')).unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("nothing to undo") && !list(s).contains("Newest"))
        .unwrap();
    assert!(list(&screen).contains("Middle"), "{screen}");
    quit(t);
    assert_eq!(server.subjects(INBOX), ["Oldest", "Middle"]);
}

// A Delete made offline waits in the Cache, across a restart, and the
// resync before it is sent does not bring the row back.
#[test]
fn a_delete_made_offline_reaches_the_server_on_the_next_run() {
    let server = server();
    let home = Home::for_server(&server);
    quit(ready(&server, &home, "Newest"));
    server.fail(Outage::Offline);

    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Body of Newest")).unwrap();
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !list(s).contains("Newest")).unwrap();
    quit(t);

    server.recover();
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Body of Middle")).unwrap();
    wait_for(|| server.subjects(TRASH) == ["Newest"]);
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    let screen = t.snapshot_after(|_| true).unwrap();
    quit(t);
    assert!(!list(&screen).contains("Newest"), "{screen}");
}

// Undone before the server saw it, a Delete never reaches the server.
#[test]
fn an_undo_while_offline_takes_the_delete_back_unsent() {
    let server = server();
    let home = Home::for_server(&server);
    quit(ready(&server, &home, "Newest"));
    server.fail(Outage::Offline);

    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Body of Newest")).unwrap();
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !list(s).contains("Newest")).unwrap();
    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| list(s).contains("Newest")).unwrap();
    quit(t);

    server.recover();
    quit(ready(&server, &home, "Newest"));
    assert!(moves(&server).is_empty());
    assert_eq!(server.subjects(INBOX).len(), 3);
}

#[test]
fn without_move_a_copy_flag_and_expunge_stand_in() {
    let server = server();
    server.without_move();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");

    t.send(Key::Backspace).unwrap();
    wait_for(|| server.subjects(TRASH) == ["Newest"]);
    wait_for(|| server.subjects(INBOX).len() == 2);
    quit(t);
    let uid = FIRST_UID + 2;
    let commands = server.commands();
    let at = |wanted: &str| commands.iter().position(|c| c == wanted).unwrap();
    let copy = at(&format!("UID COPY {uid} \"{TRASH}\""));
    let flag = at(&format!("UID STORE {uid} +FLAGS.SILENT (\\Deleted)"));
    assert!(
        copy < flag && flag < at(&format!("UID EXPUNGE {uid}")),
        "{commands:#?}"
    );
}

#[test]
fn without_a_trash_folder_the_delete_is_taken_back() {
    let server = FakeImap::start(vec![
        message("Alice", "Oldest", "2024-03-01T09:00:00Z"),
        message("Carol", "Newest", "2024-03-03T09:00:00Z"),
    ]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");

    t.send(Key::Backspace).unwrap();
    let screen = t
        .snapshot_after(|s| hint_line(s).contains("no Trash Folder") && list(s).contains("Newest"))
        .unwrap();
    quit(t);
    assert!(moves(&server).is_empty(), "{screen}");
}

/// Where each Delete the Cache holds stands, by id.
fn moved(home: &Home) -> Vec<&'static str> {
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let mut statement = cache
        .prepare("SELECT want_away, at_away, at_uid IS uid FROM moved ORDER BY id")
        .unwrap();
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)));
    rows.unwrap()
        .map(|row| match row.unwrap() {
            (true, false, _) => "queued",
            (true, true, _) => "in Trash",
            (false, true, _) => "queued back",
            (false, false, true) => "undone unsent",
            (false, false, false) => "back under a new UID",
        })
        .collect()
}

// Between an Undo and the resync after it, the restored row still has the
// UID the Message had before; a Delete then must move it by its new one.
#[test]
fn a_delete_right_after_an_undo_reaches_the_restored_message() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    t.send(Key::Backspace).unwrap();
    wait_for(|| moved(&home) == ["in Trash"]);
    server.stall(Stall::Fetch);

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| list(s).contains("Newest")).unwrap();
    wait_for(|| moved(&home) == ["back under a new UID"]);
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !list(s).contains("Newest")).unwrap();
    server.release();

    wait_for(|| server.subjects(TRASH) == ["Newest"]);
    wait_for(|| server.commands().iter().filter(|c| *c == "IDLE").count() >= 3);
    let screen = t.snapshot_after(|_| true).unwrap();
    quit(t);
    assert_eq!(server.subjects(INBOX), ["Oldest", "Middle"], "{screen}");
    assert!(!list(&screen).contains("Newest"), "{screen}");
}

// The server's EXPUNGE of a move undone meanwhile must not take the row
// the Undo put back.
#[test]
fn an_undo_while_the_move_is_under_way_keeps_the_row() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    server.stall(Stall::Move);
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !list(s).contains("Newest")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c.starts_with("UID MOVE")));

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| list(s).contains("Newest")).unwrap();
    server.release();
    wait_for(|| moved(&home).is_empty() && server.subjects(INBOX).len() == 3);
    std::thread::sleep(Duration::from_millis(300));
    let screen = t.snapshot_after(|_| true).unwrap();
    quit(t);
    assert!(list(&screen).contains("Newest"), "{screen}");
    assert!(server.subjects(TRASH).is_empty());
}

// A Delete before the server made the Undo takes the Undo back: nothing
// goes to the server, and the Message stays in Trash.
#[test]
fn a_delete_before_the_undo_is_sent_keeps_the_message_in_trash() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    t.send(Key::Backspace).unwrap();
    wait_for(|| moved(&home) == ["in Trash"]);
    server.fail(Outage::Offline);

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| list(s).contains("Newest")).unwrap();
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !list(s).contains("Newest")).unwrap();
    server.recover();
    wait_for(|| server.commands().iter().filter(|c| *c == "IDLE").count() >= 3);
    quit(t);
    assert_eq!(server.subjects(TRASH), ["Newest"]);
    assert_eq!(moves(&server).len(), 1, "{:#?}", moves(&server));
}

// A Delete while the Undo's move is under way sends the Message back to
// Trash by the UID the server gave it, once that move is answered.
#[test]
fn a_delete_while_the_undo_is_under_way_follows_it_to_trash() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Newest");
    t.send(Key::Backspace).unwrap();
    wait_for(|| moved(&home) == ["in Trash"]);
    server.stall(Stall::Move);

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| list(s).contains("Newest")).unwrap();
    wait_for(|| moves(&server).len() == 2);
    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| !list(s).contains("Newest")).unwrap();
    server.release();

    wait_for(|| moves(&server).len() == 3 && server.subjects(TRASH) == ["Newest"]);
    let screen = t.snapshot_after(|_| true).unwrap();
    quit(t);
    assert_eq!(server.subjects(INBOX), ["Oldest", "Middle"], "{screen}");
    assert!(!list(&screen).contains("Newest"), "{screen}");
}
