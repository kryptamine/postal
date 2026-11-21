//! A Draft's recipient written as words expands, once `$EDITOR` exits, to
//! the Correspondent each word begins a word of: learnt from mail in the
//! Cache and from the Folder with the Sent role. See ADR 0016.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use support::{
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
};
use tempfile::TempDir;
use termlens::{Key, Screen, Terminal};

const SENT: &str = "[Gmail]/Sent Mail";

/// `$EDITOR`: it copies `next` over the Draft, as if the user wrote it.
struct Editor {
    dir: TempDir,
}

impl Editor {
    fn writing(text: &str) -> Self {
        let editor = Self {
            dir: tempfile::tempdir().unwrap(),
        };

        editor.write(text);
        editor
    }

    fn write(&self, text: &str) {
        fs::write(self.next(), text).unwrap();
    }

    fn next(&self) -> PathBuf {
        self.dir.path().join("next")
    }

    fn command(&self) -> String {
        format!("cp '{}'", self.next().display())
    }
}

fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n\
         [smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp.port,
        fake_oauth::shared().config()
    ))
}

/// `PATH` stays, for `sh` to find the editor's commands.
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

fn from(name: &'static str, address: &'static str, subject: &str, date: &str) -> FakeMessage {
    FakeMessage {
        from_address: address,
        ..FakeMessage::new(name, subject, date)
    }
}

/// Mail from Maria Ivanova, then from Mark Twain.
fn inbox() -> FakeImap {
    FakeImap::start(vec![
        from(
            "Maria Ivanova",
            "maria@example.com",
            "Lunch",
            "2024-11-17T09:00:00Z",
        ),
        from(
            "Mark Twain",
            "mark@example.com",
            "Notes",
            "2024-11-18T09:00:00Z",
        ),
    ])
}

fn previewed(screen: &Screen) -> bool {
    screen.contains(" Draft ") && screen.contains("y send")
}

/// Started on the Message List, then `c` and the editor writing `draft`.
fn composed(home: &Home, draft: &str, listed: &str) -> (Terminal, Editor) {
    let editor = Editor::writing(draft);
    let mut t = postal(home, &editor);
    t.wait_until(|s| s.contains(listed)).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains(" Draft ")).unwrap();
    (t, editor)
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

/// The text of the one Draft.
fn draft(home: &Home) -> String {
    let mut entries = fs::read_dir(home.drafts_dir()).unwrap();
    let path = entries.next().unwrap().unwrap().path();
    assert!(entries.next().is_none());
    fs::read_to_string(path).unwrap()
}

/// The rows of the recipient picker titled for `entry`, best first, each
/// trimmed between the borders, past the Query and the rule under it.
fn choices(screen: &Screen, entry: &str) -> Vec<String> {
    let title = format!("┌ {entry} → ");
    let chars = |row: u16| screen.row_text(row).chars().collect::<Vec<_>>();
    let Some(top) = (0..screen.rows()).find(|&r| screen.row_text(r).contains(&title)) else {
        return Vec::new();
    };

    let corners = chars(top);
    let left = corners.iter().position(|&c| c == '┌').unwrap();
    let Some(right) = corners[left..].iter().position(|&c| c == '┐') else {
        return Vec::new();
    };

    (top + 3..screen.rows())
        .map(chars)
        .take_while(|row| row.get(left) == Some(&'│'))
        .map(|row| {
            row[left + 1..left + right]
                .iter()
                .collect::<String>()
                .trim()
                .to_owned()
        })
        .collect()
}

#[test]
fn a_recipient_one_correspondent_matches_is_written_out_in_the_draft() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let written = "To: mar iv\nCc: bob@example.com\nSubject: Hi\n\nHello\n";
    let (mut t, _editor) = composed(&home, written, "Notes");
    t.wait_until(|s| previewed(s) && s.contains("Maria Ivanova <maria@example.com>"))
        .unwrap();
    // An address is left as written.
    assert_eq!(
        draft(&home),
        "To: Maria Ivanova <maria@example.com>\nCc: bob@example.com\nSubject: Hi\n\nHello\n"
    );

    t.send(Key::Char('y')).unwrap();
    wait_for(|| smtp.received().len() == 1);
    quit(t);
    let received = &smtp.received()[0];
    assert_eq!(received.rcpt_to, ["maria@example.com", "bob@example.com"]);
}

#[test]
fn several_matches_are_offered_to_choose_from_the_newest_first() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let (mut t, _editor) = composed(&home, "To: ma\nSubject: Hi\n\nHello\n", "Notes");
    t.wait_until(|s| choices(s, "ma").len() == 2).unwrap();
    let screen = t.screen();
    assert_eq!(
        choices(&screen, "ma"),
        [
            "Mark Twain <mark@example.com>",
            "Maria Ivanova <maria@example.com>"
        ],
        "{screen:?}"
    );

    // The Query narrows the matches, and Enter writes the one chosen.
    t.send_str("iv").unwrap();
    t.wait_until(|s| choices(s, "ma").len() == 1).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| choices(s, "ma").is_empty() && s.contains("Maria Ivanova <maria@"))
        .unwrap();
    quit(t);
    assert!(
        draft(&home).starts_with("To: Maria Ivanova <maria@example.com>\n"),
        "{}",
        draft(&home)
    );
}

#[test]
fn a_recipient_left_unexpanded_stops_the_send() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let (mut t, _editor) = composed(&home, "To: zed, ma\nSubject: Hi\n\nHello\n", "Notes");
    t.wait_until(|s| !choices(s, "ma").is_empty()).unwrap();
    // Esc leaves the recipient as written.
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| s.contains("no Correspondent matches zed, ma"))
        .unwrap();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Draft does not parse"))
        .unwrap();
    quit(t);
    assert!(smtp.received().is_empty());
    assert!(draft(&home).starts_with("To: zed, ma\n"));
}

#[test]
fn whom_the_account_wrote_to_in_sent_ranks_above_whom_it_heard_from() {
    let (imap, smtp) = (
        FakeImap::start(vec![from(
            "Petra Svoboda",
            "petra@example.com",
            "Notes",
            "2024-11-18T09:00:00Z",
        )]),
        FakeSmtp::start(),
    );

    let sent = FakeMessage::new("Me", "Plans", "2024-01-02T09:00:00Z")
        .sent_by_me()
        .to("Petr Novak", "petr@example.com")
        .message_id("<plans@example.com>");
    imap.add_folder(SENT, &["\\Sent"], vec![sent]);
    let home = home(&imap, &smtp);
    let editor = Editor::writing("To: pet\nSubject: Hi\n\nHello\n");
    let mut t = postal(&home, &editor);
    t.wait_until(|s| s.contains("Notes")).unwrap();
    // The pass over Sent waits for the fetch connection to go quiet.
    wait_for(|| {
        fs::read_to_string(home.log_path())
            .is_ok_and(|log| log.contains(&format!("from 1 Messages of {SENT}")))
    });

    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| choices(s, "pet").len() == 2).unwrap();
    let screen = t.screen();
    assert_eq!(
        choices(&screen, "pet"),
        [
            "Petr Novak <petr@example.com>",
            "Petra Svoboda <petra@example.com>"
        ],
        "{screen:?}"
    );

    quit(t);
}

#[test]
fn the_account_itself_and_a_no_reply_address_are_never_offered() {
    let imap = FakeImap::start(vec![
        from(
            "GitHub",
            "noreply@github.com",
            "Notes",
            "2024-11-18T09:00:00Z",
        ),
        FakeMessage::new("Me", "Self", "2024-11-17T09:00:00Z").sent_by_me(),
    ]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let local = EMAIL.split('@').next().unwrap();
    let written = format!("To: git, {local}\nSubject: Hi\n\nHello\n");
    let (mut t, _editor) = composed(&home, &written, "Notes");
    t.wait_until(|s| s.contains(&format!("no Correspondent matches git, {local}")))
        .unwrap();
    quit(t);
}

#[test]
fn whom_a_send_went_to_is_a_correspondent_at_once() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let written = "To: Зоя Петрова <zoya@example.com>\nSubject: Hi\n\nHello\n";
    let (mut t, editor) = composed(&home, written, "Notes");
    t.wait_until(previewed).unwrap();
    t.send(Key::Char('y')).unwrap();
    wait_for(|| smtp.received().len() == 1);
    t.wait_until(|s| !s.contains(" Draft ")).unwrap();

    // Matched without case, in any script.
    editor.write("To: ЗО\nSubject: Again\n\nHello\n");
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| previewed(s) && s.contains("Зоя Петрова <zoya@example.com>"))
        .unwrap();
    quit(t);
}
