//! `Attach:` lines in a Draft send its files with it: read once on Send
//! into the Outbox, so a retry sends the same bytes, and a file that cannot
//! be read is refused before anything goes out.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fs, path::Path, time::Duration};

use mail_parser::{MessageParser, MimeHeaders};
use support::{
    editor::{saved, sending},
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
    picker::picked,
};
use tempfile::TempDir;
use termlens::{Key, Screen, Terminal};

/// A file no text encoding would carry unchanged.
const PDF: &[u8] = b"%PDF-1.4\n\x00\xff\xfe\r\n\x80binary\n";

const NOTES: &str = "Café notes\nline two\n";

/// `$HOME` holding the files a Draft attaches, and `$EDITOR` copying the
/// Draft as written over the one Postal opens.
struct Desk {
    home: TempDir,
}

impl Desk {
    fn new() -> Self {
        let desk = Self {
            home: tempfile::tempdir().unwrap(),
        };

        fs::create_dir_all(desk.home.path().join("Documents")).unwrap();
        fs::write(desk.home.path().join("Documents/report.pdf"), PDF).unwrap();
        fs::write(desk.home.path().join("заметки.txt"), NOTES).unwrap();
        desk
    }

    fn path(&self) -> &Path {
        self.home.path()
    }

    fn write(&self, draft: &str) {
        fs::write(self.path().join("next.eml"), draft).unwrap();
    }

    fn editor(&self) -> String {
        format!("cp '{}'", self.path().join("next.eml").display())
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

fn postal(home: &Home, desk: &Desk, editor: &str) -> Terminal {
    Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("HOME", desk.path())
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("EDITOR", editor)
        .env("TZ", "UTC")
        .env("POSTAL_SEND_DELAY", "0")
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
    screen.contains("Weekly notes")
        && !screen.contains(" Draft ")
        && !screen.contains(" Local Drafts ")
}

fn previewed(screen: &Screen) -> bool {
    screen.contains(" Draft ") && screen.contains("y send")
}

/// Started on the Message List, a Draft written with `c` and sent from
/// the editor.
fn composed(home: &Home, desk: &Desk) -> Terminal {
    let mut t = postal(home, desk, &sending(&desk.editor()));
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// Each attachment of `message`: its name, its type and its bytes decoded.
fn attachments(message: &str) -> Vec<(String, String, Vec<u8>)> {
    let parsed = MessageParser::default().parse(message.as_bytes()).unwrap();
    parsed
        .attachments()
        .map(|part| {
            let kind = part.content_type().unwrap();
            let kind = format!("{}/{}", kind.ctype(), kind.subtype().unwrap_or_default());
            let name = part.attachment_name().unwrap().to_owned();
            (name, kind, part.contents().to_vec())
        })
        .collect()
}

const TWO: &str = "To: bob@example.com\nSubject: Report\nAttach: ~/Documents/report.pdf\n\
    Attach: заметки.txt\n\nBoth attached.\n";

#[test]
fn a_draft_with_two_attach_lines_sends_both_files_byte_for_byte() {
    let (imap, smtp, desk) = (inbox(), FakeSmtp::start(), Desk::new());
    desk.write(TWO);
    let home = home(&imap, &smtp);
    let mut t = composed(&home, &desk);
    t.wait_until(|s| s.contains("Sent") && listed(s)).unwrap();
    quit(t);

    let received = smtp.received();
    assert_eq!(received.len(), 1);
    let data = &received[0].data;
    assert!(data.contains("multipart/mixed"), "{data}");
    let parsed = MessageParser::default().parse(data.as_bytes()).unwrap();
    assert_eq!(parsed.body_text(0).unwrap().trim_end(), "Both attached.");
    assert_eq!(
        attachments(data),
        [
            (
                "report.pdf".to_owned(),
                "application/pdf".to_owned(),
                PDF.to_vec()
            ),
            (
                "заметки.txt".to_owned(),
                "text/plain".to_owned(),
                NOTES.as_bytes().to_vec()
            ),
        ]
    );
    // `Attach:` is Postal's own: a server must never see it.
    assert!(!data.contains("Attach:"), "{data}");
}

// Sent bytes come from the Outbox alone: a retry must not read the files
// again, nor refuse the Message for one that is gone.
#[test]
fn a_retry_after_the_file_is_deleted_sends_the_message_as_queued() {
    let (imap, smtp, desk) = (inbox(), FakeSmtp::start(), Desk::new());
    smtp.down();
    desk.write(TWO);
    let home = home(&imap, &smtp);
    let mut t = composed(&home, &desk);
    t.wait_until(|s| listed(s) && s.contains("1 outbox"))
        .unwrap();
    fs::remove_file(desk.path().join("Documents/report.pdf")).unwrap();

    smtp.up();
    imap.deliver(FakeMessage::new("Bob", "Lunch", "2024-11-19T09:00:00Z"));
    t.wait_until(|s| s.contains("Lunch") && !s.contains("outbox"))
        .unwrap();
    quit(t);

    let received = smtp.received();
    assert_eq!(received.len(), 1);
    let files: Vec<_> = attachments(&received[0].data)
        .into_iter()
        .map(|(name, _, bytes)| (name, bytes))
        .collect();
    assert_eq!(
        files,
        [
            ("report.pdf".to_owned(), PDF.to_vec()),
            ("заметки.txt".to_owned(), NOTES.as_bytes().to_vec()),
        ]
    );
}

#[test]
fn a_missing_file_is_refused_with_its_line_named_before_anything_is_sent() {
    let (imap, smtp, desk) = (inbox(), FakeSmtp::start(), Desk::new());
    desk.write("To: bob@example.com\nSubject: Report\nAttach: ~/gone.pdf\n\nHi\n");
    let home = home(&imap, &smtp);
    let mut t = composed(&home, &desk);
    t.wait_until(|s| previewed(s) && s.contains("`Attach: ~/gone.pdf`"))
        .unwrap();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("the Draft does not parse"))
        .unwrap();
    quit(t);
    assert!(smtp.received().is_empty());
}

#[test]
fn files_over_25_mb_together_are_refused_before_sending() {
    let (imap, smtp, desk) = (inbox(), FakeSmtp::start(), Desk::new());
    let big = fs::File::create(desk.path().join("big.bin")).unwrap();
    big.set_len(20 * 1024 * 1024).unwrap();
    desk.write("To: bob@example.com\nAttach: big.bin\nAttach: big.bin\n\nHi\n");
    let home = home(&imap, &smtp);
    let mut t = composed(&home, &desk);
    t.wait_until(|s| previewed(s) && s.contains("over 25 MB"))
        .unwrap();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("the Draft does not parse"))
        .unwrap();
    quit(t);
    assert!(smtp.received().is_empty());
}

#[test]
fn a_refused_message_goes_back_to_the_drafts_with_its_attach_lines() {
    let (imap, smtp, desk) = (inbox(), FakeSmtp::start(), Desk::new());
    smtp.refuse("554 5.7.1 Message rejected by policy");
    desk.write(TWO);
    let home = home(&imap, &smtp);
    let mut t = composed(&home, &desk);
    t.wait_until(|s| listed(s) && s.contains("back in Drafts"))
        .unwrap();
    quit(t);

    let drafts: Vec<_> = fs::read_dir(home.drafts_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "eml"))
        .collect();
    assert_eq!(drafts.len(), 1);
    assert_eq!(fs::read_to_string(&drafts[0]).unwrap(), TWO);
}

#[test]
fn local_drafts_counts_the_files_a_draft_attaches() {
    let (imap, smtp, desk) = (inbox(), FakeSmtp::start(), Desk::new());
    desk.write(TWO);
    let home = home(&imap, &smtp);
    let mut t = postal(&home, &desk, &desk.editor());
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| saved(s) && listed(s)).unwrap();
    t.send(Key::Char('f')).unwrap();
    t.send_str("local").unwrap();
    t.wait_until(|s| picked(s).first().is_some_and(|f| f == "Local Drafts"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(" Local Drafts ") && s.contains("Report"))
        .unwrap();
    let screen = t.screen();
    let row = (0..screen.rows())
        .map(|row| screen.row_text(row))
        .find(|line| line.contains("Report"))
        .unwrap();
    assert!(row.trim_end_matches([' ', '│']).ends_with("+ 2"), "{row}");
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| previewed(s) && s.contains("Attach  заметки.txt"))
        .unwrap();
    quit(t);
}
