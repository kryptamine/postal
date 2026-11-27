//! With `contacts = "macos"`, the people in macOS Contacts are
//! Correspondents too: the CLI lists them, the TUI's Search completes them.
//! Contacts are faked through `POSTAL_CONTACTS_FAKE`, see ADR 0035.

#![cfg(target_os = "macos")]

use crate::support;

use std::{fs, path::Path};

use serde_json::Value;
use support::{
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    home::Home,
    quit,
};
use tempfile::TempDir;
use termlens::{Key, Terminal};

fn inbox() -> FakeImap {
    FakeImap::start(vec![FakeMessage {
        from_address: "petra@example.com",
        ..FakeMessage::new("Petra Svoboda", "Weekly notes", "2024-11-18T09:00:00Z")
    }])
}

/// A home on `imap`, with `contacts` as the first line of its config.
fn home(imap: &FakeImap, contacts: &str) -> Home {
    Home::new(&format!(
        "{contacts}\nemail = \"{EMAIL}\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\n\
         tls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        fake_oauth::shared().config()
    ))
}

/// The fake Contacts: a file of people, kept for the test's length.
fn address_book(people: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("people.json"), people).unwrap();
    dir
}

fn people_of(dir: &TempDir) -> String {
    dir.path().join("people.json").display().to_string()
}

const PEOPLE: &str = r#"[
    {"name": "Zed Zimmer", "emails": ["zed@example.com"]},
    {"name": "Petra (work)", "emails": ["Petra@Example.com"]},
    {"name": "Anna Berg", "emails": ["anna@example.com", "berg@example.com"]},
    {"name": "Me", "emails": ["ME@example.com"]}
]"#;

fn postal(home: &Home, fake: &str) -> Terminal {
    home.terminal()
        .size(100, 30)
        .env("POSTAL_CONTACTS_FAKE", fake)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

/// The Inbox synced into the Cache, for the CLI to read.
fn synced(home: &Home) {
    let mut t = postal(home, "denied");
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    quit(t);
}

/// `postal correspondent list --json` with Contacts faked by `fake`, as
/// `(address, name, written)` best first.
fn listed(home: &Home, fake: &str) -> Vec<(String, Option<String>, bool)> {
    let output = home
        .command(env!("CARGO_BIN_EXE_postal"))
        .args(["correspondent", "list", "--json"])
        .env("POSTAL_CONTACTS_FAKE", fake)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    rows.iter()
        .map(|row| {
            (
                row["address"].as_str().unwrap().to_owned(),
                row["name"].as_str().map(str::to_owned),
                row["written"].as_bool().unwrap(),
            )
        })
        .collect()
}

fn row(address: &str, name: &str) -> (String, Option<String>, bool) {
    (address.to_owned(), Some(name.to_owned()), false)
}

// The name the user gave wins over the one mail uses, and those known from
// Contacts alone follow everyone with mail, by name, the Account left out.
#[test]
fn contacts_rename_correspondents_and_follow_them_by_name() {
    let imap = inbox();
    let home = home(&imap, "contacts = \"macos\"");
    synced(&home);
    let book = address_book(PEOPLE);
    assert_eq!(
        listed(&home, &people_of(&book)),
        [
            row("petra@example.com", "Petra (work)"),
            row("anna@example.com", "Anna Berg"),
            row("berg@example.com", "Anna Berg"),
            row("zed@example.com", "Zed Zimmer"),
        ]
    );
}

// Contacts are read only once the user asks for them.
#[test]
fn no_contacts_while_the_switch_is_off() {
    let imap = inbox();
    let home = home(&imap, "");
    synced(&home);
    let book = address_book(PEOPLE);
    assert_eq!(
        listed(&home, &people_of(&book)),
        [row("petra@example.com", "Petra Svoboda")]
    );
}

// A refused permission is never an error that stops a completion.
#[test]
fn a_denied_permission_falls_back_to_mail() {
    let imap = inbox();
    let home = home(&imap, "contacts = \"macos\"");
    synced(&home);
    assert_eq!(
        listed(&home, "denied"),
        [row("petra@example.com", "Petra Svoboda")]
    );
}

// The TUI's Search completes a person from Contacts by their name.
#[test]
fn search_completes_a_person_from_contacts() {
    let imap = inbox();
    let home = home(&imap, "contacts = \"macos\"");
    let book = address_book(PEOPLE);
    let mut t = postal(&home, &people_of(&book));
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    t.send(Key::Char('/')).unwrap();
    t.send_str("from:zim").unwrap();
    t.wait_until(|s| s.contains("zed@example.com") && s.contains("Zed Zimmer"))
        .unwrap();
    quit(t);
}

fn config(home: &Home) -> String {
    fs::read_to_string(home.config_path()).unwrap()
}

fn log_says(path: &Path, text: &str) -> bool {
    fs::read_to_string(path).is_ok_and(|log| log.contains(text))
}

// Turning the switch on reads Contacts at once, so macOS asks then; a
// refusal says so in a Toast, and Correspondents come from mail alone.
#[test]
fn turning_contacts_on_toasts_a_denied_permission() {
    let imap = inbox();
    let home = home(&imap, "");
    let mut t = postal(&home, "denied");
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    t.send(Key::Char(',')).unwrap();
    t.wait_until(|s| s.contains("Settings")).unwrap();
    t.send(Key::BackTab).unwrap();
    t.send(Key::BackTab).unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("an agent through MCP sees them too"))
        .unwrap();
    assert!(screen.contains("none ✓"), "{screen}");
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("macos ✓")).unwrap();
    t.wait_until(|s| s.contains("access to Contacts is denied"))
        .unwrap();
    assert!(config(&home).contains("contacts = \"macos\""));
    assert!(log_says(&home.log_path(), "access to Contacts is denied"));
    quit(t);
}
