//! `y` moves a Draft to the Outbox, which the sync connection sends: a
//! Message waits there while the fake SMTP server is down, goes back to the
//! Drafts when refused, and is never sent twice once accepted.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use mail_parser::MessageParser;
use support::{
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
};
use tempfile::TempDir;
use termlens::{Key, Screen, Terminal};

const WRITTEN: &str = "To: Bob Example <bob@example.com>\nSubject: Plans\n\nHi Bob\n";

const REFUSAL: &str = "554 5.7.1 Message rejected by policy";

/// `$EDITOR`, copying `WRITTEN` over the Draft.
struct Editor {
    dir: TempDir,
}

impl Editor {
    fn new() -> Self {
        let editor = Self {
            dir: tempfile::tempdir().unwrap(),
        };

        fs::write(editor.dir.path().join("next"), WRITTEN).unwrap();
        editor
    }

    fn command(&self) -> String {
        format!("cp '{}'", self.dir.path().join("next").display())
    }
}

fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp.port,
        fake_oauth::shared().config()
    ))
}

fn postal(home: &Home, editor: &Editor) -> Terminal {
    Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("EDITOR", editor.command())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn inbox() -> FakeImap {
    FakeImap::start(vec![FakeMessage::new(
        "Ada Lovelace",
        "Weekly notes",
        "2024-11-18T09:00:00Z",
    )])
}

fn listed(screen: &Screen) -> bool {
    screen.contains("Weekly notes") && !screen.contains(" Draft ")
}

/// Started on the Message List, a Draft written and `y` sent on its preview.
fn sent(home: &Home, editor: &Editor) -> Terminal {
    let mut t = postal(home, editor);
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains(" Draft ") && s.contains("y send"))
        .unwrap();
    t.send(Key::Char('y')).unwrap();
    t
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

/// The files in `dir`, by name.
fn files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<_> = entries.map(|entry| entry.unwrap().path()).collect();
    files.sort();
    files
}

fn message_id(message: &str) -> String {
    let parsed = MessageParser::default().parse(message.as_bytes()).unwrap();
    parsed.message_id().unwrap().to_owned()
}

/// The Message waiting in the Outbox; there is one.
fn waiting(home: &Home) -> String {
    let files = files(&home.outbox_dir());
    let messages: Vec<_> = files
        .iter()
        .filter(|path| path.extension().is_some_and(|e| e == "eml"))
        .collect();
    assert_eq!(messages.len(), 1, "{files:?}");
    fs::read_to_string(messages[0]).unwrap()
}

#[test]
fn a_message_sent_while_smtp_is_down_waits_in_the_outbox_until_it_is_back() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    smtp.down();
    let (home, editor) = (home(&imap, &smtp), Editor::new());
    let mut t = sent(&home, &editor);
    t.wait_until(|s| listed(s) && s.contains("1 outbox"))
        .unwrap();
    assert!(files(&home.drafts_dir()).is_empty());
    let id = message_id(&waiting(&home));

    // New mail ends the IDLE, and the sync after it sends the Outbox again.
    smtp.up();
    imap.deliver(FakeMessage::new("Bob", "Lunch", "2024-11-19T09:00:00Z"));
    t.wait_until(|s| s.contains("Lunch") && !s.contains("outbox"))
        .unwrap();
    quit(t);

    let received = smtp.received();
    assert_eq!(received.len(), 1);
    assert_eq!(message_id(&received[0].data), id, "the Message as queued");
    assert!(files(&home.outbox_dir()).is_empty());
}

#[test]
fn a_message_left_in_the_outbox_is_sent_after_a_restart() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    smtp.down();
    let (home, editor) = (home(&imap, &smtp), Editor::new());
    let mut t = sent(&home, &editor);
    t.wait_until(|s| listed(s) && s.contains("1 outbox"))
        .unwrap();
    quit(t);
    let id = message_id(&waiting(&home));

    smtp.up();
    let mut t = postal(&home, &editor);
    t.wait_until(|s| listed(s) && s.contains("Sent")).unwrap();
    assert!(!t.screen().contains("outbox"));
    quit(t);

    let received = smtp.received();
    assert_eq!(received.len(), 1);
    assert_eq!(message_id(&received[0].data), id, "the Message as queued");
    assert!(files(&home.outbox_dir()).is_empty());
}

#[test]
fn a_refused_message_goes_back_to_the_drafts_with_the_server_answer() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    smtp.refuse(REFUSAL);
    let (home, editor) = (home(&imap, &smtp), Editor::new());
    let mut t = sent(&home, &editor);
    t.wait_until(|s| listed(s) && s.contains("refused · back in Drafts"))
        .unwrap();
    assert!(!t.screen().contains("outbox"));
    quit(t);

    assert!(smtp.received().is_empty());
    assert!(files(&home.outbox_dir()).is_empty());
    let drafts = files(&home.drafts_dir());
    assert_eq!(drafts.len(), 2, "the Draft and its notes: {drafts:?}");
    assert_eq!(fs::read_to_string(&drafts[0]).unwrap(), WRITTEN);
    let notes = fs::read_to_string(&drafts[1]).unwrap();
    assert!(notes.contains("Message rejected by policy"), "{notes}");
}

// The copy in Sent is a step of its own: a connection lost on it must not
// hand the Message to SMTP again once the sync reconnects.
#[test]
fn a_message_smtp_accepted_is_never_sent_again_after_its_copy_fails() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    imap.without_gmail();
    imap.add_folder("Sent", &["\\Sent"], Vec::new());
    imap.cut_appends(true);
    let (home, editor) = (home(&imap, &smtp), Editor::new());
    let t = sent(&home, &editor);
    wait_for(|| imap.commands().iter().any(|c| c.starts_with("APPEND ")));
    assert_eq!(smtp.received().len(), 1);

    imap.cut_appends(false);
    wait_for(|| !imap.appended().is_empty());
    wait_for(|| files(&home.outbox_dir()).is_empty());
    quit(t);

    let (received, appended) = (smtp.received(), imap.appended());
    assert_eq!(received.len(), 1);
    assert_eq!(appended.len(), 1);
    assert_eq!(appended[0].0, "Sent");
    assert_eq!(message_id(&appended[0].1), message_id(&received[0].data));
}
