//! Compose with `c` writes a Draft and opens `$EDITOR` on it; its preview
//! sends it with `y` to the fake SMTP server, edits it again with `e`, or
//! discards it with `q`, which `u` takes back.

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

use mail_parser::{Address, MessageParser};
use support::{
    fake_imap::{ACCESS_TOKEN, EMAIL, FakeImap, FakeMessage, PASSWORD, xoauth2},
    fake_oauth::{FakeOAuth, REFRESH_TOKEN},
    fake_smtp::FakeSmtp,
    home::Home,
};
use tempfile::TempDir;
use termlens::{Key, Screen, Terminal};

const NAME: &str = "Zoë Example";

/// A Draft as the user writes it: a long line, which is sent unwrapped,
/// and a Subject that is not ASCII.
const WRITTEN: &str = "To: Bob Example <bob@example.com>\nCc: carol@example.com\n\
    Subject: Café plans\n\nHi Bob,\n\nThis line runs well past seventy-eight \
    columns, and it must reach the server as one line.\n";

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

fn smtp_config(smtp: &FakeSmtp) -> String {
    format!(
        "\n[smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n",
        smtp.port
    )
}

/// An app-password Account with a display name, on both fake servers.
fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\nname = \"{NAME}\"\nauth = \"app-password\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n{}\n[secrets]\nstore = \"file\"\n",
        imap.port,
        smtp_config(smtp)
    ))
}

/// `PATH` stays, for `sh` to find the editor's commands.
fn postal(home: &Home, editor: &str) -> Terminal {
    Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("EDITOR", editor)
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

fn previewed(screen: &Screen) -> bool {
    screen.contains(" Draft ") && screen.contains("y send")
}

/// Started on the Message List, then `c` and the editor, on the preview.
fn composed(home: &Home, editor: &Editor) -> Terminal {
    let mut t = postal(home, &editor.command());
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(previewed).unwrap();
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

/// The Drafts' files with what each holds.
fn drafts(home: &Home) -> Vec<(PathBuf, String)> {
    let Ok(entries) = fs::read_dir(home.drafts_dir()) else {
        return Vec::new();
    };

    entries
        .map(|entry| {
            let path = entry.unwrap().path();
            let text = fs::read_to_string(&path).unwrap();
            (path, text)
        })
        .collect()
}

fn mailbox(address: Option<&Address<'_>>) -> Vec<(Option<String>, String)> {
    address
        .unwrap()
        .iter()
        .map(|a| {
            let name = a.name().map(str::to_owned);
            (name, a.address().unwrap().to_owned())
        })
        .collect()
}

#[test]
fn a_sent_draft_reaches_the_server_as_written_with_the_headers_postal_adds() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let editor = Editor::writing(WRITTEN);
    let mut t = composed(&home, &editor);
    let screen = t.screen();
    assert!(screen.contains(&format!("{NAME} <{EMAIL}>")), "{screen:?}");
    assert!(screen.contains("Café plans"), "{screen:?}");
    assert_eq!(drafts(&home).len(), 1);

    imap.clear_commands();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Sent") && listed(s)).unwrap();
    // The Message shows in Sent only once a sync has read it there.
    wait_for(|| imap.commands().iter().any(|c| c.starts_with("SELECT ")));
    quit(t);
    // Gmail saves the copy in Sent itself.
    assert!(!imap.commands().iter().any(|c| c.starts_with("APPEND ")));

    assert_eq!(
        smtp.logins(),
        [("PLAIN".to_owned(), format!("\0{EMAIL}\0{PASSWORD}"))]
    );
    let received = smtp.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].mail_from, EMAIL);
    assert_eq!(
        received[0].rcpt_to,
        ["bob@example.com", "carol@example.com"]
    );

    let data = &received[0].data;
    let head = data.split("\r\n\r\n").next().unwrap();
    assert!(head.is_ascii(), "a raw non-ASCII header: {head}");
    assert!(
        head.contains("Content-Type: text/plain; charset=utf-8\r\n"),
        "{head}"
    );

    let message = MessageParser::default().parse(data.as_bytes()).unwrap();
    assert_eq!(
        mailbox(message.from()),
        [(Some(NAME.to_owned()), EMAIL.to_owned())]
    );
    assert_eq!(
        mailbox(message.to()),
        [(Some("Bob Example".to_owned()), "bob@example.com".to_owned())]
    );
    assert_eq!(
        mailbox(message.cc()),
        [(None, "carol@example.com".to_owned())]
    );
    assert_eq!(message.subject(), Some("Café plans"));
    assert!(message.date().is_some(), "{head}");
    let id = message.message_id().unwrap();
    assert!(id.ends_with("@example.com"), "{id}");
    let body = WRITTEN.split_once("\n\n").unwrap().1;
    assert_eq!(
        message.body_text(0).unwrap(),
        body.replace('\n', "\r\n"),
        "decoded, each line is as written, the long one too"
    );
    assert!(!head.contains("flowed"), "{head}");
    assert!(drafts(&home).is_empty());
}

#[test]
fn an_oauth_account_sends_with_xoauth2() {
    let (imap, smtp, oauth) = (inbox(), FakeSmtp::start(), FakeOAuth::start());
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\nauth = \"oauth\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n{}\n[secrets]\nstore = \"file\"\n\n[oauth]\ntoken-url = \"{}\"\n",
        imap.port,
        smtp_config(&smtp),
        oauth.token_url()
    ));
    fs::write(home.secret_path(), format!("{REFRESH_TOKEN}\n")).unwrap();
    let editor = Editor::writing(WRITTEN);
    let mut t = composed(&home, &editor);
    // Without a name in the config, `From` is the bare address.
    let screen = t.screen();
    assert!(screen.contains(&format!("From    {EMAIL}")), "{screen:?}");

    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Sent") && listed(s)).unwrap();
    quit(t);

    assert_eq!(smtp.logins(), [("XOAUTH2".to_owned(), xoauth2())]);
    assert!(xoauth2().contains(ACCESS_TOKEN));
    assert_eq!(smtp.received().len(), 1);
}

/// `y` on a Draft written as `text`, whose preview shows `error`: nothing
/// reaches the server, and the file stays as written.
fn refused(text: &str, error: &str) {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let editor = Editor::writing(text);
    let mut t = composed(&home, &editor);
    let screen = t.screen();
    assert!(screen.contains(error), "{screen:?}");

    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("the Draft does not parse") && previewed(s))
        .unwrap();
    quit(t);

    assert!(smtp.logins().is_empty());
    assert!(smtp.received().is_empty());
    let drafts = drafts(&home);
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].1, text, "the file is left as written");
}

#[test]
fn an_empty_to_refuses_send() {
    refused("To: \nSubject: Hello\n\nBody\n", "`To` is empty");
}

#[test]
fn a_bad_address_refuses_send() {
    refused(
        "To: bob at example\nSubject: Hello\n\nBody\n",
        "`To` holds a bad address",
    );
}

// An editor quit without a change is no cancel: the template stays a Draft.
#[test]
fn an_unchanged_draft_is_previewed_too() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let mut t = postal(&home, "true");
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| previewed(s) && s.contains("`To` is empty"))
        .unwrap();
    quit(t);

    assert_eq!(drafts(&home)[0].1, "To: \nCc: \nSubject: \n\n");
}

#[test]
fn e_opens_the_editor_again_on_the_same_draft() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let editor = Editor::writing("To: \n\n");
    let mut t = composed(&home, &editor);
    assert!(t.screen().contains("`To` is empty"));

    editor.write(WRITTEN);
    t.send(Key::Char('e')).unwrap();
    t.wait_until(|s| previewed(s) && s.contains("Café plans"))
        .unwrap();
    assert!(!t.screen().contains("`To` is empty"));
    quit(t);

    let drafts = drafts(&home);
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].1, WRITTEN);
}

#[test]
fn q_discards_the_draft_and_u_brings_it_back() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let editor = Editor::writing(WRITTEN);
    let mut t = composed(&home, &editor);
    let path = drafts(&home)[0].0.clone();

    t.send(Key::Char('q')).unwrap();
    t.wait_until(|s| s.contains("Discarded · u undo") && listed(s))
        .unwrap();
    assert!(drafts(&home).is_empty());

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| previewed(s) && s.contains("Café plans"))
        .unwrap();
    assert_eq!(drafts(&home), [(path, WRITTEN.to_owned())]);

    // The Draft brought back sends as any other.
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Sent") && listed(s)).unwrap();
    quit(t);
    assert_eq!(smtp.received().len(), 1);
}

#[test]
fn compose_works_from_reading_too() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let editor = Editor::writing(WRITTEN);
    let mut t = postal(&home, &editor.command());
    t.wait_until(listed).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("n/p next")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(previewed).unwrap();
    quit(t);
    assert_eq!(drafts(&home).len(), 1);
}
