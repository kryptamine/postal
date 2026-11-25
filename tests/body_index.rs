//! Every cached Message's body joins the search index in the background,
//! so a word past the Snippet finds it, offline too.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    process::Output,
    time::{Duration, Instant},
};

use serde_json::Value;
use support::{
    fake_body::FakeBody,
    fake_imap::{FIRST_UID, FakeImap, FakeMessage, Stall},
    home::Home,
    picker::wait_picked,
    search::{hint_line, list, postal, quit, wait_selected},
};
use termlens::{Key, Terminal};

/// Longer than a Snippet, so a word after it is in the body alone.
fn past_the_snippet(word: &str) -> String {
    format!(
        "{} {word} at the end",
        "Filler words before it. ".repeat(12)
    )
}

/// `count` Messages a minute apart, the newest last, so the List shows UID
/// `FIRST_UID + count - 1` first; Message `n` says `wordnx` past its Snippet,
/// a word no other one starts with.
fn messages(count: u32) -> Vec<FakeMessage> {
    (0..count)
        .map(|n| {
            let date = format!("2024-03-01T{:02}:{:02}:00Z", n / 60, n % 60);
            FakeMessage::new("Sender", &format!("Subject {n}"), &date)
                .body(FakeBody::plain(&past_the_snippet(&format!("word{n}x"))))
        })
        .collect()
}

fn command(home: &Home, args: &[&str]) -> Output {
    home.command(env!("CARGO_BIN_EXE_postal"))
        .args(args)
        .output()
        .unwrap()
}

/// The subjects `postal message search` finds for `query`.
fn found(home: &Home, query: &str) -> Vec<String> {
    let output = command(home, &["message", "search", query, "--json"]);
    assert!(output.status.success(), "{output:?}");
    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    rows.iter()
        .map(|row| row["subject"].as_str().unwrap().to_owned())
        .collect()
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn idling(server: &FakeImap) -> bool {
    server.commands().iter().any(|c| c == "IDLE")
}

/// A fetch of the body pass: a text part, cut at its index length.
fn indexing(command: &str) -> bool {
    command.contains("<0.32768>")
}

fn started(server: &FakeImap, home: &Home) -> Terminal {
    let mut t = postal(home);
    t.wait_until(|s| s.contains("Subject")).unwrap();
    wait_for(|| idling(server));
    t
}

#[test]
fn a_word_only_in_a_body_is_found_and_stays_found_offline() {
    let server = FakeImap::start(messages(3));
    let home = Home::for_server(&server);
    let mut t = started(&server, &home);

    wait_for(|| found(&home, "word1x") == ["Subject 1"]);
    t.send(Key::Char('/')).unwrap();
    t.send_str("word2x").unwrap();
    let screen = t.snapshot_after(|s| list(s).contains("Subject 2")).unwrap();
    assert!(!list(&screen).contains("Subject 1"), "{screen}");
    quit(t);

    // The CLI reads the Cache alone, as an offline Postal does.
    drop(server);
    assert_eq!(found(&home, "word0x"), ["Subject 0"]);
}

#[test]
fn mail_arriving_after_the_pass_is_indexed_too() {
    let server = FakeImap::start(messages(2));
    let home = Home::for_server(&server);
    let t = started(&server, &home);
    wait_for(|| found(&home, "word0x") == ["Subject 0"]);

    server.deliver(
        FakeMessage::new("Sender", "Late", "2024-04-01T09:00:00Z")
            .body(FakeBody::plain(&past_the_snippet("latecomer"))),
    );
    wait_for(|| found(&home, "latecomer") == ["Late"]);
    quit(t);
}

#[test]
fn an_expunged_message_leaves_the_index() {
    let server = FakeImap::start(messages(2));
    let home = Home::for_server(&server);
    let t = started(&server, &home);
    wait_for(|| found(&home, "word0x") == ["Subject 0"]);

    server.expunge(FIRST_UID);
    wait_for(|| found(&home, "word0x").is_empty());
    quit(t);
}

/// The body pass gives way between batches: a Preview asked for while one
/// is on the wire goes before the next.
#[test]
fn a_preview_waits_on_one_batch_at_most() {
    const COUNT: u32 = 200;
    let server = FakeImap::start(messages(COUNT));
    let home = Home::for_server(&server);
    let mut t = started(&server, &home);

    server.stall(Stall::Part);
    wait_for(|| server.commands().iter().any(|c| indexing(c)));
    let held = server.commands().iter().rposition(|c| indexing(c)).unwrap();
    for _ in 0..12 {
        t.send(Key::Char('j')).unwrap();
    }

    // Row 12 from the top, newest first.
    let selected = FIRST_UID + COUNT - 1 - 12;
    server.release();
    t.wait_until(|s| s.contains("word187x")).unwrap();
    let commands = server.commands();
    let preview = commands
        .iter()
        .position(|c| c.starts_with(&format!("UID FETCH {selected} ")))
        .unwrap_or_else(|| panic!("{commands:#?}"));
    assert!(
        !commands[held + 1..preview].iter().any(|c| indexing(c)),
        "{commands:#?}"
    );
    quit(t);
}

fn plans() -> FakeMessage {
    FakeMessage::new("Ada", "Engine plans", "2024-02-01T09:00:00Z")
        .body(FakeBody::plain(&past_the_snippet("difference")))
}

#[test]
fn a_message_in_a_folder_never_opened_is_found_then_shown() {
    let server = FakeImap::start(messages(1));
    server.add_folder("Projects", &[], vec![plans()]);
    let home = Home::for_server(&server);
    let mut t = started(&server, &home);

    wait_for(|| found(&home, "difference") == ["Engine plans"]);
    t.send(Key::Char('/')).unwrap();
    t.send_str("difference").unwrap();
    wait_selected(&mut t, "Engine plans");
    // Its body comes from the Folder it is in.
    t.wait_until(|s| s.contains("difference at the end"))
        .unwrap();
    quit(t);
}

// The Picker's count of a Folder walked whole is newer than its rows, which
// only a resync brings up to it: the index must not wait on that Message.
#[test]
fn a_folder_counted_after_its_walk_leaves_the_index_whole() {
    let server = FakeImap::start(messages(1));
    server.add_folder("Projects", &[], vec![plans()]);
    let home = Home::for_server(&server);
    let mut t = started(&server, &home);
    wait_for(|| found(&home, "difference") == ["Engine plans"]);

    server.deliver_to(
        "Projects",
        FakeMessage::new("Ada", "Later plans", "2024-02-02T09:00:00Z"),
    );
    wait_picked(&mut t, "Projects 2");
    quit(t);

    server.clear_commands();
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Subject")).unwrap();
    t.send(Key::Char('/')).unwrap();
    t.send_str("zeppelin").unwrap();
    // The walk lists the Folders, then counts how far the index is.
    wait_for(|| server.commands().iter().any(|c| c.starts_with("LIST")));
    let screen = t.wait_stable(Duration::from_secs(1)).unwrap();
    assert!(hint_line(&screen).contains("/zeppelin"), "{screen}");
    assert!(!hint_line(&screen).contains("index"), "{screen}");
    quit(t);
}

// The first sync and a Backfill fetch the newest 500; the rest of the
// Folder comes in the background.
#[test]
fn a_message_past_the_newest_page_is_found() {
    let server = FakeImap::start(messages(600));
    let home = Home::for_server(&server);
    let t = started(&server, &home);

    wait_for(|| found(&home, "word3x") == ["Subject 3"]);
    quit(t);
}

#[test]
fn trash_and_spam_are_left_on_the_server() {
    let server = FakeImap::start(messages(1));
    for (name, attribute) in [("Trash", "\\Trash"), ("Spam", "\\Junk")] {
        server.add_folder(
            name,
            &[attribute],
            vec![
                FakeMessage::new("Eve", name, "2024-02-01T09:00:00Z")
                    .body(FakeBody::plain("binned")),
            ],
        );
    }

    // Listed after them, so the pass is past them once it is found.
    server.add_folder("Projects", &[], vec![plans()]);
    let home = Home::for_server(&server);
    let t = started(&server, &home);

    wait_for(|| found(&home, "difference") == ["Engine plans"]);
    quit(t);
    let examined = |name: &str| {
        server
            .commands()
            .iter()
            .any(|c| c.starts_with("EXAMINE") && c.contains(name))
    };
    assert!(
        !examined("Trash") && !examined("Spam"),
        "{:#?}",
        server.commands()
    );
}

// All Mail holds every label's Messages: walking each label too would
// download them twice.
#[test]
fn on_gmail_all_mail_alone_is_walked() {
    let server = FakeImap::start(messages(1));
    server.add_folder("Projects", &[], vec![plans()]);
    server.add_all_mail("[Gmail]/All Mail");
    let home = Home::for_server(&server);
    let t = started(&server, &home);

    wait_for(|| found(&home, "difference") == ["Engine plans"]);
    quit(t);
    assert!(
        !server.commands().iter().any(|c| c.contains("Projects")),
        "{:#?}",
        server.commands()
    );
}

// Outside Gmail a body key is UIDVALIDITY and UID, which two Folders may
// share: one Folder's body must not be indexed under the other's Message.
#[test]
fn folders_sharing_a_uidvalidity_keep_their_own_bodies() {
    let server = FakeImap::start(vec![
        FakeMessage::new("Sender", "Inbox one", "2024-03-01T09:00:00Z")
            .body(FakeBody::plain(&past_the_snippet("alphaword"))),
    ]);
    server.without_gmail();
    server.add_folder(
        "Projects",
        &[],
        vec![
            FakeMessage::new("Sender", "Projects one", "2024-02-01T09:00:00Z")
                .body(FakeBody::plain(&past_the_snippet("betaword"))),
        ],
    );
    let home = Home::for_server(&server);
    let t = started(&server, &home);

    wait_for(|| found(&home, "betaword") == ["Projects one"]);
    assert_eq!(found(&home, "alphaword"), ["Inbox one"]);
    quit(t);
}

#[test]
fn a_match_in_a_body_alone_comes_after_one_on_the_card() {
    let server = FakeImap::start(vec![
        FakeMessage::new("Sender", "Lunch", "2024-03-02T09:00:00Z")
            .body(FakeBody::plain(&past_the_snippet("zeppelinword"))),
        FakeMessage::new("Sender", "Zeppelinword plans", "2024-03-01T09:00:00Z")
            .body(FakeBody::plain("Nothing else")),
    ]);
    let home = Home::for_server(&server);
    let mut t = started(&server, &home);
    wait_for(|| found(&home, "zeppelinword").len() == 2);

    t.send(Key::Char('/')).unwrap();
    t.send_str("zeppelinword").unwrap();
    let screen = t
        .snapshot_after(|s| list(s).contains("Lunch") && list(s).contains("plans"))
        .unwrap();
    let list = list(&screen);
    assert!(
        list.find("Zeppelinword plans") < list.find("Lunch"),
        "{screen}"
    );
    quit(t);
}

#[test]
fn new_mail_arrives_by_idle_during_the_body_pass() {
    let server = FakeImap::start(messages(100));
    let home = Home::for_server(&server);
    let mut t = started(&server, &home);

    server.stall(Stall::Index);
    wait_for(|| server.commands().iter().any(|c| indexing(c)));
    server.deliver(FakeMessage::new("Sender", "Late", "2024-04-01T09:00:00Z"));
    t.wait_until(|s| list(s).contains("Late")).unwrap();
    server.release();
    quit(t);
}

// A word not found yet may only be waiting for its body, or its Message.
#[test]
fn the_hint_line_shows_how_far_the_index_is_while_a_search_is_open() {
    let server = FakeImap::start(messages(600));
    let home = Home::for_server(&server);
    let mut t = started(&server, &home);

    server.stall(Stall::Part);
    t.send(Key::Char('/')).unwrap();
    t.send_str("zeppelin").unwrap();
    // The 100 past the newest page count before they are cached.
    let screen = t
        .snapshot_after(|s| hint_line(s).contains(" of 600)"))
        .unwrap();
    assert!(
        hint_line(&screen).contains("/zeppelin  search index "),
        "{screen}"
    );

    server.release();
    t.wait_until(|s| hint_line(s).contains("/zeppelin") && !hint_line(s).contains("index"))
        .unwrap();
    quit(t);
}
