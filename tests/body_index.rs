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
    search::{list, postal, quit},
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
/// `FIRST_UID + count - 1` first; Message `n` says `wordn` past its Snippet.
fn messages(count: u32) -> Vec<FakeMessage> {
    (0..count)
        .map(|n| {
            let date = format!("2024-03-01T{:02}:{:02}:00Z", n / 60, n % 60);
            FakeMessage::new("Sender", &format!("Subject {n}"), &date)
                .body(FakeBody::plain(&past_the_snippet(&format!("word{n}"))))
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

    wait_for(|| found(&home, "word1") == ["Subject 1"]);
    t.send(Key::Char('/')).unwrap();
    t.send_str("word2").unwrap();
    let screen = t.snapshot_after(|s| list(s).contains("Subject 2")).unwrap();
    assert!(!list(&screen).contains("Subject 1"), "{screen}");
    quit(t);

    // The CLI reads the Cache alone, as an offline Postal does.
    drop(server);
    assert_eq!(found(&home, "word0"), ["Subject 0"]);
}

#[test]
fn mail_arriving_after_the_pass_is_indexed_too() {
    let server = FakeImap::start(messages(2));
    let home = Home::for_server(&server);
    let t = started(&server, &home);
    wait_for(|| found(&home, "word0") == ["Subject 0"]);

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
    wait_for(|| found(&home, "word0") == ["Subject 0"]);

    server.expunge(FIRST_UID);
    wait_for(|| found(&home, "word0").is_empty());
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
    t.wait_until(|s| s.contains("word187")).unwrap();
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
