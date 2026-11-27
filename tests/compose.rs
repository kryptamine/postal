//! Compose with `c` writes a Draft and opens `$EDITOR` on it, which sends
//! it to the fake SMTP server as `:PostalSend` does, or only saves it; its
//! preview sends it with `y`, edits it again with `e`, or discards it with
//! `q`, which `u` takes back.

use crate::support;

use std::{fs, path::PathBuf};

use mail_parser::{Address, MessageParser};
use support::{
    editor::{preview_newest, saved, sending},
    fake_imap::{ACCESS_TOKEN, EMAIL, FakeImap, FakeMessage, xoauth2},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
    picker::picked,
    quit, wait_for,
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

    /// The editor ended with `:PostalSend`.
    fn sends(&self) -> String {
        sending(&self.command())
    }
}

fn smtp_config(smtp: &FakeSmtp) -> String {
    format!(
        "\n[smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n",
        smtp.port
    )
}

/// An Account with a display name, on both fake servers.
fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    named_home(imap, smtp, &format!("name = \"{NAME}\"\n"))
}

/// An Account with the `name` line given, on both fake servers.
fn named_home(imap: &FakeImap, smtp: &FakeSmtp, name: &str) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n{name}\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n{}\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp_config(smtp),
        fake_oauth::shared().config()
    ))
}

/// `PATH` stays, for `sh` to find the editor's commands.
fn postal(home: &Home, editor: &str) -> Terminal {
    home.terminal()
        .size(100, 30)
        .env("EDITOR", editor)
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
    screen.contains("Weekly notes") && !screen.contains(" Draft ") && !local(screen)
}

fn local(screen: &Screen) -> bool {
    screen.contains(" Local Drafts ")
}

fn previewed(screen: &Screen) -> bool {
    screen.contains(" Draft ") && screen.contains("y send")
}

/// Started on the Message List, then `c` and the editor `command`.
fn composed(home: &Home, command: &str) -> Terminal {
    let mut t = postal(home, command);
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t
}

/// [`composed`] with an editor that only saves, then the Draft's preview.
fn previewing(home: &Home, editor: &Editor) -> Terminal {
    let mut t = composed(home, &editor.command());
    preview_newest(&mut t);
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

fn quit_saved(mut t: Terminal) {
    t.wait_until(saved).unwrap();
    quit(t);
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
    let mut t = postal(&home, &editor.sends());
    t.wait_until(listed).unwrap();
    imap.clear_commands();
    t.send(Key::Char('c')).unwrap();
    // Straight from the editor: no preview on the way.
    t.wait_until(|s| s.contains("Sent") && listed(s)).unwrap();
    // The Message shows in Sent only once a sync has read it there.
    wait_for(|| imap.commands().iter().any(|c| c.starts_with("SELECT ")));
    quit(t);
    // Gmail saves the copy in Sent itself.
    assert!(!imap.commands().iter().any(|c| c.starts_with("APPEND ")));

    assert_eq!(smtp.logins(), [("XOAUTH2".to_owned(), xoauth2())]);
    assert!(xoauth2().contains(ACCESS_TOKEN));
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
    assert_eq!(drafts(&home), Vec::<(PathBuf, String)>::new());
}

#[test]
fn without_a_name_from_is_the_bare_address() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = named_home(&imap, &smtp, "");
    let editor = Editor::writing(WRITTEN);
    let mut t = previewing(&home, &editor);
    let screen = t.screen();
    assert!(screen.contains(&format!("From    {EMAIL}")), "{screen:?}");

    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Sent") && local(s)).unwrap();
    quit(t);

    assert_eq!(smtp.received().len(), 1);
}

/// The editor's Send of a Draft written as `text` opens its preview, which
/// shows `error`, and so does `y` there: nothing reaches the server, and
/// the file stays as written.
fn refused(text: &str, error: &str) {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let editor = Editor::writing(text);
    let mut t = composed(&home, &editor.sends());
    t.wait_until(|s| s.contains("the Draft does not parse") && previewed(s))
        .unwrap();
    let screen = t.screen();
    assert!(screen.contains(error), "{screen:?}");

    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("the Draft does not parse") && previewed(s))
        .unwrap();
    quit(t);

    assert_eq!(smtp.logins(), Vec::<(String, String)>::new());
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

// An editor quit on the bare template was a `c` pressed by mistake: no
// preview, and no Draft left behind in Local Drafts.
#[test]
fn an_untouched_compose_leaves_no_draft() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let blanked = Editor::writing("To:\nCc:  \nSubject:\n\n\n");
    for editor in ["true".to_owned(), blanked.command()] {
        let mut t = postal(&home, &editor);
        t.wait_until(listed).unwrap();
        t.send(Key::Char('c')).unwrap();
        t.wait_until(|s| s.contains("Empty Draft dropped") && !previewed(s))
            .unwrap();
        quit(t);

        assert!(drafts(&home).is_empty(), "{editor}");
    }
}

#[test]
fn e_opens_the_editor_again_on_the_same_draft() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let editor = Editor::writing("To: \nSubject: Hello\n\n");
    let mut t = previewing(&home, &editor);
    assert!(t.screen().contains("`To` is empty"));

    // An editor that only saves comes back to the preview it left.
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
fn the_editor_opened_by_e_sends_the_draft_too() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let editor = Editor::writing(WRITTEN);
    let t = composed(&home, &editor.command());
    quit_saved(t);

    let mut t = postal(&home, &editor.sends());
    t.wait_until(listed).unwrap();
    open_local(&mut t);
    t.send(Key::Enter).unwrap();
    t.wait_until(previewed).unwrap();
    t.send(Key::Char('e')).unwrap();
    t.wait_until(|s| s.contains("Sent") && local(s) && !previewed(s))
        .unwrap();
    quit(t);
    assert_eq!(smtp.received().len(), 1);
    assert_eq!(drafts(&home), Vec::<(PathBuf, String)>::new());
}

#[test]
fn d_discards_the_draft_and_u_brings_it_back() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let editor = Editor::writing(WRITTEN);
    let mut t = previewing(&home, &editor);
    let path = drafts(&home)[0].0.clone();

    t.send(Key::Char('d')).unwrap();
    t.wait_until(|s| s.contains("Discarded · u undo") && local(s))
        .unwrap();
    assert_eq!(drafts(&home), Vec::<(PathBuf, String)>::new());

    t.send(Key::Char('u')).unwrap();
    t.wait_until(|s| previewed(s) && s.contains("Café plans"))
        .unwrap();
    assert_eq!(drafts(&home), [(path, WRITTEN.to_owned())]);

    // The Draft brought back sends as any other.
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Sent") && local(s)).unwrap();
    quit(t);
    assert_eq!(smtp.received().len(), 1);
}

// The user read the mail while writing it: the editor's plain exit keeps
// the Draft for Local Drafts, and shows it no second time.
#[test]
fn an_editor_that_only_saves_keeps_the_draft_with_no_preview() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let editor = Editor::writing(WRITTEN);
    let mut t = composed(&home, &editor.command());
    t.wait_until(|s| saved(s) && listed(s)).unwrap();
    quit(t);

    assert!(smtp.received().is_empty());
    assert_eq!(drafts(&home)[0].1, WRITTEN);
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
    t.wait_until(saved).unwrap();
    quit(t);
    assert_eq!(drafts(&home).len(), 1);
}
