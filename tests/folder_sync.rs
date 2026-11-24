//! `postal folder sync` brings the Cache up to the server with no TUI open,
//! sending what the TUI left queued first, and steps aside for a TUI that
//! keeps the Cache live itself. See ADR 0033.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fs, process::Output};

use serde_json::{Value, json};
use support::{
    fake_body::FakeBody,
    fake_imap::{EMAIL, FakeImap, FakeMessage, Outage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
};
use termlens::Key;

fn inbox() -> FakeImap {
    FakeImap::start(vec![
        FakeMessage {
            from_address: "ada@example.com",
            ..FakeMessage::new("Ada Lovelace", "Weekly notes", "2024-11-18T09:00:00Z")
        }
        .body(FakeBody::plain("The notes")),
        FakeMessage::new("Bob", "Invoice 1042", "2024-11-16T09:00:00Z")
            .body(FakeBody::plain("Please pay by Friday")),
    ])
}

fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp.port,
        fake_oauth::shared().config()
    ))
}

fn postal(home: &Home, args: &[&str]) -> Output {
    home.command(env!("CARGO_BIN_EXE_postal"))
        .args(args)
        .output()
        .unwrap()
}

/// The `--json` answer to `args`, which succeeded.
fn json(home: &Home, args: &[&str]) -> Vec<Value> {
    let mut args = args.to_vec();
    args.push("--json");
    let output = postal(home, &args);
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn synced(new: usize) -> Value {
    json!([{
        "account": EMAIL.to_lowercase(),
        "folder": "INBOX",
        "new": new,
        "outcome": "synced",
        "error": null,
    }])
}

#[test]
fn a_sync_into_an_empty_cache_brings_the_inbox_with_bodies_to_read_and_search() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    assert_eq!(
        json(&home, &["folder", "sync"]),
        synced(2).as_array().unwrap()[..]
    );

    let listed = json(&home, &["message", "list"]);
    let subjects: Vec<_> = listed.iter().map(|row| &row["subject"]).collect();
    assert_eq!(subjects, ["Weekly notes", "Invoice 1042"]);
    let shown = json(
        &home,
        &["message", "show", listed[1]["message"].as_str().unwrap()],
    );
    assert_eq!(shown[0]["body"], "Please pay by Friday");
    // A word of the body alone: the sync indexed it.
    let found = json(&home, &["message", "search", "friday"]);
    assert_eq!(found[0]["subject"], "Invoice 1042");

    let output = postal(&home, &["folder", "sync"]);
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text, format!("{} INBOX: 0 new\n", EMAIL.to_lowercase()));
}

#[test]
fn a_sync_of_a_stale_cache_brings_what_came_since() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    json(&home, &["folder", "sync"]);
    imap.deliver(
        FakeMessage::new("Carol", "Lunch", "2024-11-19T09:00:00Z")
            .body(FakeBody::plain("Noon at the usual place")),
    );

    assert_eq!(
        json(&home, &["folder", "sync"]),
        synced(1).as_array().unwrap()[..]
    );
    let listed = json(&home, &["message", "list"]);
    assert_eq!(listed[0]["subject"], "Lunch");
    let shown = json(
        &home,
        &["message", "show", listed[0]["message"].as_str().unwrap()],
    );
    assert_eq!(shown[0]["body"], "Noon at the usual place");
}

// What the TUI queued and could not send goes out with no TUI open.
#[test]
fn a_sync_sends_the_message_the_tui_left_in_the_outbox() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    smtp.down();
    let home = home(&imap, &smtp);
    let editor = tempfile::tempdir().unwrap();
    let written = editor.path().join("next");
    fs::write(&written, "To: bob@example.com\nSubject: Plans\n\nHi Bob\n").unwrap();
    let mut t = (home.terminal())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("EDITOR", format!("cp '{}'", written.display()))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(|s| s.contains("Invoice 1042")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains("y send")).unwrap();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("1 outbox")).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert!(smtp.received().is_empty());

    smtp.up();
    json(&home, &["folder", "sync"]);
    let received = smtp.received();
    assert_eq!(received.len(), 1);
    assert!(
        received[0].data.contains("Subject: Plans"),
        "{}",
        received[0].data
    );
    assert!(fs::read_dir(home.outbox_dir()).unwrap().next().is_none());
}

// Two processes sending one Outbox could send a Message twice.
#[test]
fn a_sync_steps_aside_for_an_open_tui() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let mut t = home.terminal().spawn(env!("CARGO_BIN_EXE_postal")).unwrap();
    t.wait_until(|s| s.contains("Invoice 1042")).unwrap();

    let rows = json(&home, &["folder", "sync"]);
    assert_eq!(rows[0]["outcome"], "open");
    assert!(rows[0]["new"].is_null());
    let output = postal(&home, &["folder", "sync"]);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.ends_with(": Postal is open and keeps it live\n"),
        "{text}"
    );

    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn a_signed_out_account_fails_the_sync() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    fs::remove_file(home.secret_path()).unwrap();

    let output = postal(&home, &["folder", "sync", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows[0]["outcome"], "signed-out");
}

// An agent or a cron job tells an outage from success by the exit code alone.
#[test]
fn a_server_outage_fails_the_sync() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    imap.fail(Outage::Offline);

    let output = postal(&home, &["folder", "sync", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows[0]["outcome"], "failed");
}
