//! `d` keeps a Draft in its preview, and Local Drafts in the Folder Picker
//! lists every Draft and Outbox Message to take up again; a Reply to a
//! Message already answered by a Draft opens that Draft.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fs, path::PathBuf, time::Duration};

use support::{
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
    picker::picked,
};
use termlens::{Key, Screen, Terminal};

const WRITTEN: &str = "To: Bob Example <bob@example.com>\nSubject: Café plans\n\nHi Bob\n";

fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp.port,
        fake_oauth::shared().config()
    ))
}

/// `$EDITOR` copies `WRITTEN` over the Draft, as if the user wrote it.
fn postal(home: &Home) -> Terminal {
    let next = home.xdg_config_home().join("next");
    fs::write(&next, WRITTEN).unwrap();
    Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("EDITOR", format!("cp '{}'", next.display()))
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn inbox() -> FakeImap {
    FakeImap::start(vec![
        FakeMessage {
            from_address: "ada@example.com",
            ..FakeMessage::new("Ada Lovelace", "Weekly notes", "2024-11-18T09:00:00Z")
        }
        .header("Message-ID", "<notes@example.com>"),
    ])
}

fn listed(screen: &Screen) -> bool {
    screen.contains("Weekly notes") && !screen.contains(" Draft ") && !local(screen)
}

fn previewed(screen: &Screen) -> bool {
    screen.contains(" Draft ") && screen.contains("y send")
}

fn local(screen: &Screen) -> bool {
    screen.contains(" Local Drafts ")
}

/// Started on the Message List, a Draft written with `c` and kept with `d`.
fn kept(home: &Home) -> Terminal {
    let mut t = postal(home);
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(previewed).unwrap();
    t.send(Key::Char('d')).unwrap();
    t.wait_until(|s| s.contains("Saved to Drafts") && listed(s))
        .unwrap();
    t
}

fn open_local(t: &mut Terminal) {
    t.send(Key::Char('f')).unwrap();
    t.send_str("local").unwrap();
    t.wait_until(|s| picked(s).first().is_some_and(|f| f == "Local Drafts"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(local).unwrap();
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// The Drafts' files, notes left out.
fn drafts(home: &Home) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(home.drafts_dir()) else {
        return Vec::new();
    };

    entries
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "eml"))
        .collect()
}

#[test]
fn a_kept_draft_shows_in_local_drafts_after_a_restart_and_sends_from_there() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    quit(kept(&home));
    assert_eq!(drafts(&home).len(), 1);

    let mut t = postal(&home);
    t.wait_until(listed).unwrap();
    open_local(&mut t);
    t.wait_until(|s| s.contains("bob@example.com") && s.contains("Café plans"))
        .unwrap();

    t.send(Key::Enter).unwrap();
    t.wait_until(|s| previewed(s) && s.contains("Hi Bob"))
        .unwrap();
    t.send(Key::Char('y')).unwrap();
    // Sent, the Draft leaves the list it was opened from.
    t.wait_until(|s| s.contains("Sent") && local(s) && !s.contains("Café plans"))
        .unwrap();
    quit(t);
    assert_eq!(smtp.received().len(), 1);
}

#[test]
fn a_second_reply_to_the_same_message_opens_the_first_draft() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let mut t = postal(&home);
    t.wait_until(listed).unwrap();
    t.send(Key::Char('r')).unwrap();
    t.wait_until(previewed).unwrap();
    t.send(Key::Char('d')).unwrap();
    t.wait_until(|s| s.contains("Saved to Drafts") && listed(s))
        .unwrap();
    quit(t);
    let first = drafts(&home);
    assert_eq!(first.len(), 1);

    // After a restart too, and for Reply All as for Reply.
    let mut t = postal(&home);
    t.wait_until(listed).unwrap();
    t.send(Key::Char('R')).unwrap();
    t.wait_until(previewed).unwrap();
    t.send(Key::Char('d')).unwrap();
    t.wait_until(listed).unwrap();
    quit(t);
    assert_eq!(drafts(&home), first);
}

#[test]
fn a_cache_schema_change_leaves_the_drafts_in_place() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    quit(kept(&home));
    let before = drafts(&home);

    fs::remove_file(home.cache_path()).unwrap();
    let old = rusqlite::Connection::open(home.cache_path()).unwrap();
    old.execute_batch("CREATE TABLE rows (x); PRAGMA user_version = 0;")
        .unwrap();
    drop(old);

    let mut t = postal(&home);
    t.wait_until(|s| s.contains("cache rebuilt")).unwrap();
    t.wait_until(listed).unwrap();
    open_local(&mut t);
    t.wait_until(|s| s.contains("Café plans")).unwrap();
    quit(t);
    assert_eq!(drafts(&home), before);
}

#[test]
fn an_outbox_message_shows_read_only_with_its_last_error() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    smtp.down();
    let home = home(&imap, &smtp);
    let mut t = postal(&home);
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(previewed).unwrap();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| listed(s) && s.contains("1 outbox"))
        .unwrap();

    open_local(&mut t);
    t.wait_until(|s| s.contains("Outbox") && s.contains("Café plans"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| s.contains(" Outbox ") && s.contains("Hi Bob"))
        .unwrap();
    assert!(screen.contains("SMTP: "), "{screen}");
    assert!(!screen.contains("y send"), "{screen}");

    // Read-only: no key sends, edits or discards it.
    for key in ['y', 'e', 'd'] {
        t.send(Key::Char(key)).unwrap();
    }

    t.send(Key::Char('q')).unwrap();
    t.wait_until(|s| local(s) && !s.contains("Hi Bob")).unwrap();
    quit(t);
    assert!(drafts(&home).is_empty());
    assert!(smtp.received().is_empty());
}

#[test]
fn backspace_discards_a_draft_in_local_drafts_and_u_brings_it_back() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let mut t = kept(&home);
    let before = drafts(&home);
    open_local(&mut t);
    t.wait_until(|s| s.contains("Café plans")).unwrap();

    t.send(Key::Backspace).unwrap();
    t.wait_until(|s| s.contains("Discarded · u undo") && s.contains("no Drafts"))
        .unwrap();
    assert!(drafts(&home).is_empty());

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| previewed(s) && s.contains("Café plans"))
        .unwrap();
    assert_eq!(drafts(&home), before);
    t.send(Key::Char('d')).unwrap();
    t.wait_until(|s| local(s) && s.contains("Café plans") && !previewed(s))
        .unwrap();
    quit(t);
}
