//! Several Accounts in one run: each syncs on its own connections, the
//! Folder Picker lists a section per Account, and what is done to a
//! Message reaches the server of the Account it belongs to.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};

use support::{
    fake_body::FakeBody,
    fake_imap::{APP_PASSWORD, EMAIL, FakeImap, FakeMessage, Outage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
    picker::{picked, picker},
};
use tempfile::TempDir;
use termlens::{Key, Screen, Terminal};

/// The second Account, at iCloud.
const OTHER: &str = "me@icloud.example";

const WRITTEN: &str = "To: Bob Example <bob@example.com>\nSubject: Plans\n\nHi Bob\n";

/// The first Account at a Gmail-like server, the second at an iCloud-like
/// one, each sending through its own SMTP server.
struct Two {
    first: FakeImap,
    second: FakeImap,
    /// The UID of the second Account's one Message.
    bravo: u32,
    smtp: (FakeSmtp, FakeSmtp),
    home: Home,
    editor: TempDir,
}

fn two() -> Two {
    let first = FakeImap::start(vec![
        FakeMessage::new("Alice", "Alpha news", "2024-03-02T09:00:00Z")
            .body(FakeBody::plain("Body of Alpha news")),
    ]);
    let second = FakeImap::start(Vec::new());
    let bravo = second.deliver(
        FakeMessage::new("Bob", "Bravo notes", "2024-03-01T09:00:00Z")
            .body(FakeBody::plain("Body of Bravo notes"))
            .unread(),
    );
    second.as_icloud();
    let smtp = (FakeSmtp::start(), FakeSmtp::start());
    let server = |port: u16| format!("host = \"127.0.0.1\"\nport = {port}\ntls = false\n");
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\n{}\n[smtp]\n{}\n[secrets]\nstore = \"file\"\n{}\n\
         [[account]]\nemail = \"{OTHER}\"\nprovider = \"icloud\"\n\n\
         [account.imap]\n{}\n[account.smtp]\n{}",
        server(first.port),
        server(smtp.0.port),
        fake_oauth::shared().config(),
        server(second.port),
        server(smtp.1.port),
    ));
    let secret = home.secret_path_of(OTHER);
    fs::write(&secret, format!("{APP_PASSWORD}\n")).unwrap();
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
    let editor = tempfile::tempdir().unwrap();
    fs::write(editor.path().join("next"), WRITTEN).unwrap();
    Two {
        first,
        second,
        bravo,
        smtp,
        home,
        editor,
    }
}

fn postal(two: &Two) -> Terminal {
    let editor = format!("cp '{}'", two.editor.path().join("next").display());
    Terminal::builder()
        .size(120, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", two.home.xdg_config_home())
        .env("XDG_STATE_HOME", two.home.xdg_state_home())
        .env("XDG_CACHE_HOME", two.home.xdg_cache_home())
        .env("XDG_DATA_HOME", two.home.xdg_data_home())
        .env("EDITOR", editor)
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

/// The Folder Picker's rows once both Accounts have listed their Folders.
fn both_listed(screen: &Screen) -> bool {
    let rows = picked(screen);
    rows.iter().any(|r| r == OTHER) && rows.iter().any(|r| r.starts_with("Trash"))
}

/// Shows the second Account's Inbox, the row under the first's.
fn second_inbox(t: &mut Terminal) {
    t.wait_until(|s| s.contains("Alpha news")).unwrap();
    t.send(Key::Char('f')).unwrap();
    t.wait_until(both_listed).unwrap();
    t.send(Key::Down).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| picker(s).is_empty() && s.contains("Bravo notes"))
        .unwrap();
}

#[test]
fn the_picker_heads_a_section_per_account_and_lists_local_drafts_last() {
    let two = two();
    let mut t = postal(&two);
    t.wait_until(|s| s.contains("Alpha news")).unwrap();
    t.send(Key::Char('f')).unwrap();
    let screen = t.snapshot_after(both_listed).unwrap();
    let rows = picked(&screen);
    let at = |row: &str| rows.iter().position(|r| r == row).unwrap();
    assert_eq!(rows[0], EMAIL, "{rows:?}");
    assert!(rows[1].starts_with("Inbox"), "{rows:?}");
    assert!(at(OTHER) > 1, "{rows:?}");
    assert!(rows[at(OTHER) + 1].starts_with("Inbox"), "{rows:?}");
    assert_eq!(rows.last().map(String::as_str), Some("Local Drafts"));

    // Typing narrows every section, and each keeps its heading.
    t.send_str("inb").unwrap();
    let screen = t.snapshot_after(|s| picked(s).len() == 4).unwrap();
    let rows = picked(&screen);
    assert_eq!([&*rows[0], &*rows[2]], [EMAIL, OTHER], "{rows:?}");
    assert!(rows[1].starts_with("Inbox") && rows[3].starts_with("Inbox"));
    quit(t);
}

#[test]
fn a_folder_of_the_second_account_names_it_in_the_preview_and_marks_read_there() {
    let two = two();
    let mut t = postal(&two);
    second_inbox(&mut t);
    t.wait_until(|s| s.contains("Body of Bravo notes") && s.contains(&format!("Account {OTHER}")))
        .unwrap();

    // Reading names it on each unfolded Message.
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| !s.contains("enter read") && s.contains(&format!("Account {OTHER}")))
        .unwrap();
    wait_for(|| two.second.seen("INBOX", two.bravo));
    assert!(
        !two.first.commands().iter().any(|c| c.contains("STORE")),
        "{:?}",
        two.first.commands()
    );
    quit(t);
}

#[test]
fn with_one_account_no_header_names_it() {
    let server = FakeImap::start(vec![
        FakeMessage::new("Alice", "Alpha news", "2024-03-02T09:00:00Z")
            .body(FakeBody::plain("Body of Alpha news")),
    ]);
    let home = Home::for_server(&server);
    let mut t = Terminal::builder()
        .size(120, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("Body of Alpha news"))
        .unwrap();
    assert!(!screen.contains("Account"), "{screen:?}");
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| !s.contains("enter read") && s.contains("Body of Alpha news"))
        .unwrap();
    assert!(!screen.contains("Account"), "{screen:?}");
    quit(t);
}

#[test]
fn a_delete_goes_to_the_trash_of_the_messages_account() {
    let two = two();
    let mut t = postal(&two);
    second_inbox(&mut t);
    t.send(Key::Backspace).unwrap();
    wait_for(|| two.second.subjects("Deleted Messages") == ["Bravo notes"]);
    assert_eq!(two.first.subjects("INBOX"), ["Alpha news"]);

    t.send(Key::Char('u')).unwrap();
    wait_for(|| two.second.subjects("INBOX") == ["Bravo notes"]);
    quit(t);
}

#[test]
fn a_search_matches_the_messages_of_every_account() {
    let two = two();
    let mut t = postal(&two);
    t.wait_until(|s| s.contains("Alpha news")).unwrap();
    // The second Account's rows are in the Cache once its Inbox is.
    t.send(Key::Char('f')).unwrap();
    t.wait_until(both_listed).unwrap();
    t.send(Key::Esc).unwrap();
    t.send(Key::Char('/')).unwrap();
    t.send_str("bravo").unwrap();
    t.wait_until(|s| s.contains("Bravo notes") && !s.contains("Alpha news"))
        .unwrap();
    quit(t);
}

#[test]
fn compose_and_reply_go_out_from_the_account_of_the_shown_folder() {
    let two = two();
    let mut t = postal(&two);
    second_inbox(&mut t);
    for key in ['c', 'r'] {
        t.send(Key::Char(key)).unwrap();
        t.wait_until(|s| s.contains(" Draft ") && s.contains(OTHER))
            .unwrap();
        t.send(Key::Char('y')).unwrap();
        t.wait_until(|s| s.contains("Sent") && !s.contains(" Draft "))
            .unwrap();
    }

    let received = two.smtp.1.received();
    assert_eq!(received.len(), 2, "{received:?}");
    assert!(
        received.iter().all(|r| r.mail_from == OTHER),
        "{received:?}"
    );
    assert!(two.smtp.0.received().is_empty());
    // The answered Message is flagged on the server it reached.
    wait_for(|| two.second.answered("INBOX", two.bravo));
    quit(t);
}

#[test]
fn local_drafts_names_the_sender_of_each_draft() {
    let two = two();
    let mut t = postal(&two);
    second_inbox(&mut t);
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains(" Draft ")).unwrap();
    t.send(Key::Char('d')).unwrap();
    t.send(Key::Char('f')).unwrap();
    t.wait_until(both_listed).unwrap();
    t.send_str("local").unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(" Local Drafts ") && s.contains(OTHER) && s.contains("Plans"))
        .unwrap();
    quit(t);
}

#[test]
fn the_status_names_the_account_in_the_worst_state() {
    let two = two();
    two.second.fail(Outage::Offline);
    let mut t = postal(&two);
    t.wait_until(|s| s.contains("Alpha news") && s.contains(&format!("{OTHER} · offline")))
        .unwrap();
    quit(t);
}

#[test]
fn esc_on_a_lost_secret_runs_without_that_account() {
    let two = two();
    fs::remove_file(two.home.secret_path_of(OTHER)).unwrap();
    let mut t = postal(&two);
    t.wait_until(|s| s.contains(&format!("Sign in to iCloud as {OTHER}")))
        .unwrap();
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| s.contains("Alpha news")).unwrap();
    t.send(Key::Char('f')).unwrap();
    let screen = t
        .snapshot_after(|s| picked(s).iter().any(|r| r == "Local Drafts"))
        .unwrap();
    assert!(!picked(&screen).iter().any(|r| r == OTHER), "{screen:?}");
    assert!(two.second.commands().is_empty());
    quit(t);
}
