//! The editor's Send, as `:PostalSend` ends it, moves a Draft to the
//! Outbox, which the sync connection sends: a
//! Message waits there while the fake SMTP server is down, goes back to the
//! Drafts when refused, and is never sent twice once accepted. A Send is
//! held for `send-delay` first, and `u` takes it back inside that window.

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
    time::Duration,
};

use mail_parser::MessageParser;
use support::{
    editor::sending,
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
    quit, wait_for,
};
use tempfile::TempDir;
use termlens::{Key, Screen, Terminal};

const WRITTEN: &str = "To: Bob Example <bob@example.com>\nSubject: Plans\n\nHi Bob\n";

const REFUSAL: &str = "554 5.7.1 Message rejected by policy";

/// `$EDITOR`, copying `WRITTEN` over the Draft and asking for its Send.
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
        sending(&format!("cp '{}'", self.dir.path().join("next").display()))
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
    holding(home, editor, "0")
}

/// Postal with its Sends held for `delay` seconds.
fn holding(home: &Home, editor: &Editor, delay: &str) -> Terminal {
    home.terminal()
        .size(100, 30)
        .env("EDITOR", editor.command())
        .env("POSTAL_SEND_DELAY", delay)
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

/// Started on the Message List, a Draft written and sent from the editor.
fn sent(home: &Home, editor: &Editor) -> Terminal {
    send(postal(home, editor))
}

/// A Draft written in `t` and sent from the editor, started on the
/// Message List.
fn send(mut t: Terminal) -> Terminal {
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t
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
    assert_eq!(files(&home.drafts_dir()), Vec::<PathBuf>::new());
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
    assert_eq!(files(&home.outbox_dir()), Vec::<PathBuf>::new());
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
    assert_eq!(files(&home.outbox_dir()), Vec::<PathBuf>::new());
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
    assert_eq!(files(&home.outbox_dir()), Vec::<PathBuf>::new());
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

fn held(screen: &Screen) -> bool {
    screen.contains("u to undo")
}

#[test]
fn a_held_message_reaches_smtp_once_its_window_is_over() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let (home, editor) = (home(&imap, &smtp), Editor::new());
    let mut t = send(holding(&home, &editor, "1.5"));
    t.wait_until(|s| held(s) && s.contains("sent in ")).unwrap();
    assert!(smtp.received().is_empty(), "sent inside the window");

    t.wait_until(|s| s.contains("Sent") && !held(s)).unwrap();
    quit(t);
    assert_eq!(smtp.received().len(), 1);
    assert_eq!(files(&home.outbox_dir()), Vec::<PathBuf>::new());
}

#[test]
fn u_inside_the_window_gives_the_draft_back_and_nothing_is_sent() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let (home, editor) = (home(&imap, &smtp), Editor::new());
    let mut t = send(holding(&home, &editor, "2"));
    t.wait_until(held).unwrap();
    // Sent from the editor, it comes back to a preview all the same.
    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| s.contains(" Draft ") && s.contains("y send"))
        .unwrap();

    // Past the window, which a Message left in the Outbox would have left on.
    std::thread::sleep(Duration::from_millis(2500));
    assert!(smtp.received().is_empty());
    assert_eq!(files(&home.outbox_dir()), Vec::<PathBuf>::new());
    // One Account and no Reply: the Draft has no notes to give back.
    let drafts = files(&home.drafts_dir());
    assert_eq!(drafts.len(), 1, "{drafts:?}");
    assert_eq!(fs::read_to_string(&drafts[0]).unwrap(), WRITTEN);

    // Taken back, it is a Draft like any other: sent again, it leaves once.
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Sent") && !held(s)).unwrap();
    quit(t);
    assert_eq!(smtp.received().len(), 1);
}

#[test]
fn u_after_the_send_leaves_the_outbox_alone() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let (home, editor) = (home(&imap, &smtp), Editor::new());
    let mut t = send(holding(&home, &editor, "0.3"));
    t.wait_until(|s| s.contains("Sent") && !held(s)).unwrap();
    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| s.contains("nothing to undo")).unwrap();
    quit(t);

    assert_eq!(smtp.received().len(), 1);
    assert_eq!(files(&home.drafts_dir()), Vec::<PathBuf>::new());
    assert_eq!(files(&home.outbox_dir()), Vec::<PathBuf>::new());
}

#[test]
fn a_held_message_survives_a_restart_and_is_sent_once() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let (home, editor) = (home(&imap, &smtp), Editor::new());
    let mut t = send(holding(&home, &editor, "2"));
    t.wait_until(held).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let text = t.screen().text();
    assert!(text.contains("leaves on the next sync"), "{text}");
    assert!(smtp.received().is_empty());
    let id = message_id(&waiting(&home));

    let mut t = holding(&home, &editor, "2");
    t.wait_until(|s| listed(s) && s.contains("Sent")).unwrap();
    quit(t);
    let received = smtp.received();
    assert_eq!(received.len(), 1);
    assert_eq!(message_id(&received[0].data), id, "the Message as queued");
    assert_eq!(files(&home.outbox_dir()), Vec::<PathBuf>::new());
}

#[test]
fn local_drafts_shows_how_long_a_held_send_has_left() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let (home, editor) = (home(&imap, &smtp), Editor::new());
    let mut t = send(holding(&home, &editor, "8"));
    t.wait_until(held).unwrap();
    t.send(Key::Char('f')).unwrap();
    t.send_str("local").unwrap();
    t.wait_until(|s| s.contains("Local Drafts")).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(" Local Drafts ") && s.contains("Plans") && s.contains(" in "))
        .unwrap();
    let row = t.screen().text();
    let row = row.lines().find(|line| line.contains("Plans")).unwrap();
    assert!(
        ["in 8s", "in 7s", "in 6s"]
            .iter()
            .any(|left| row.contains(left)),
        "{row}"
    );
    quit(t);
}
