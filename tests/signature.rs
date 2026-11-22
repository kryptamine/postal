//! An Account's signature from `config.toml` is in the Draft the editor
//! opens, under a `-- ` line, and a received one shows dimmed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

use mail_parser::MessageParser;
use support::{
    fake_body::FakeBody,
    fake_imap::{APP_PASSWORD, EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
};
use tempfile::TempDir;
use termlens::{Key, Screen, Terminal};

const OTHER: &str = "me@icloud.example";

/// How the quote's first line names Ada's Message.
const SAID: &str = "On Mon, 18 Nov 2024 09:00, Ada Lovelace wrote:";

/// What the editor leaves in a file that is no Draft: the signature typed.
const TYPED: &str = "Zoë Example\nPostal\n";

/// `$EDITOR`: it keeps a copy of the file as it opened, then fills in the
/// recipient and the subject a Compose leaves empty; any other file, an
/// Account's signature, it writes as `TYPED`.
struct Editor {
    dir: TempDir,
}

impl Editor {
    fn new() -> Self {
        let editor = Self {
            dir: tempfile::tempdir().unwrap(),
        };

        fs::write(editor.dir.path().join("typed"), TYPED).unwrap();
        let script = format!(
            "cd '{dir}'\ncp \"$1\" opened 2>/dev/null || : > opened\n\
             if grep -q '^Subject:' opened; then\n\
             sed -e 's/^To: $/To: bob@example.com/' -e 's/^Subject: $/Subject: Hello/' \
             opened > \"$1\"\nelse\ncp typed \"$1\"\nfi\n",
            dir = editor.dir.path().display()
        );
        fs::write(editor.script(), script).unwrap();
        editor
    }

    fn script(&self) -> PathBuf {
        self.dir.path().join("editor.sh")
    }

    fn opened_path(&self) -> PathBuf {
        self.dir.path().join("opened")
    }

    /// The file as the editor last opened it.
    fn opened(&self) -> String {
        fs::read_to_string(self.opened_path()).unwrap()
    }

    fn command(&self) -> String {
        format!("sh '{}'", self.script().display())
    }
}

/// An Account on both fake servers, with `top` among its top-level keys.
fn home(imap: &FakeImap, smtp: &FakeSmtp, top: &str) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n{top}\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n\
         [smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp.port,
        fake_oauth::shared().config()
    ))
}

fn postal(home: &Home, editor: &str) -> Terminal {
    Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("HOME", home.xdg_config_home())
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("EDITOR", editor)
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
        .header("Message-ID", "<notes@example.com>")
        .body(FakeBody::plain("Line one\r\n")),
    ])
}

fn listed(screen: &Screen) -> bool {
    screen.contains("Weekly notes") && !screen.contains(" Draft ")
}

fn previewed(screen: &Screen) -> bool {
    screen.contains(" Draft ") && screen.contains("y send")
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// `key` on the Message List, then `y` on the Draft it opens: the text
/// body the server received.
fn sent(home: &Home, smtp: &FakeSmtp, editor: &Editor, key: char) -> String {
    let mut t = postal(home, &editor.command());
    t.wait_until(listed).unwrap();
    t.send(Key::Char(key)).unwrap();
    t.wait_until(previewed).unwrap();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Sent") && listed(s)).unwrap();
    quit(t);
    let received = smtp.received();
    assert_eq!(received.len(), 1);
    let message = MessageParser::default()
        .parse(received[0].data.as_bytes())
        .unwrap();
    message.body_text(0).unwrap().into_owned()
}

#[test]
fn a_compose_opens_on_the_signature_and_the_sent_message_keeps_it() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp, "signature = \"Zoë Example\\nPostal\"\n");
    let editor = Editor::new();
    let body = sent(&home, &smtp, &editor, 'c');

    assert_eq!(
        editor.opened(),
        "To: \nCc: \nSubject: \n\n\n-- \nZoë Example\nPostal\n"
    );
    // The separator keeps its space, which is what makes it one.
    assert_eq!(body, "\r\n-- \r\nZoë Example\r\nPostal\r\n");
}

#[test]
fn without_a_signature_a_compose_opens_on_the_bare_template() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp, "");
    let editor = Editor::new();
    let body = sent(&home, &smtp, &editor, 'c');

    assert_eq!(editor.opened(), "To: \nCc: \nSubject: \n\n");
    assert_eq!(body, "");
}

#[test]
fn a_reply_holds_the_signature_above_the_quote() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp, "signature = \"Zoë Example\"\n");
    let editor = Editor::new();
    let body = sent(&home, &smtp, &editor, 'r');

    assert_eq!(
        editor.opened(),
        format!(
            "To: Ada Lovelace <ada@example.com>\nCc: \nSubject: Re: Weekly notes\n\n\n\
             -- \nZoë Example\n\n{SAID}\n> Line one\n"
        )
    );
    assert_eq!(
        body,
        format!("\r\n-- \r\nZoë Example\r\n\r\n{SAID}\r\n> Line one\r\n")
    );
}

// The file is read on each Compose, without the blank lines around its text.
#[test]
fn a_signature_file_is_beside_the_config() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp, "signature-file = \"signature.txt\"\n");
    let file = home.config_path().with_file_name("signature.txt");
    fs::write(file, "\nZoë Example\r\nPostal\n\n").unwrap();
    let editor = Editor::new();
    sent(&home, &smtp, &editor, 'c');

    assert_eq!(
        editor.opened(),
        "To: \nCc: \nSubject: \n\n\n-- \nZoë Example\nPostal\n"
    );
}

#[test]
fn a_signature_file_may_be_under_the_home_directory() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp, "signature-file = \"~/signature.txt\"\n");
    fs::write(
        home.xdg_config_home().join("signature.txt"),
        "Zoë Example\n",
    )
    .unwrap();
    let editor = Editor::new();
    sent(&home, &smtp, &editor, 'c');

    assert_eq!(
        editor.opened(),
        "To: \nCc: \nSubject: \n\n\n-- \nZoë Example\n"
    );
}

// A Draft still opens without the signature its file no longer holds.
#[test]
fn a_signature_file_that_does_not_read_is_said_and_left_out() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp, "signature-file = \"gone.txt\"\n");
    let editor = Editor::new();
    let mut t = postal(&home, &editor.command());
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| previewed(s) && s.contains("could not read the signature"))
        .unwrap();
    quit(t);

    assert_eq!(editor.opened(), "To: \nCc: \nSubject: \n\n");
}

// The signature is Postal's writing, not the user's: `c` pressed by mistake
// leaves no Draft behind.
#[test]
fn an_untouched_compose_with_a_signature_leaves_no_draft() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp, "signature = \"Zoë Example\"\n");
    let mut t = postal(&home, "true");
    t.wait_until(listed).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains("Empty Draft dropped") && !previewed(s))
        .unwrap();
    quit(t);

    let left = fs::read_dir(home.drafts_dir()).unwrap().count();
    assert_eq!(left, 0);
}

// The top level signs for the Account it names alone; another signs as its
// own table says, which is where Settings write what is typed for it.
#[test]
fn each_account_has_its_own_signature() {
    let (first, smtp) = (inbox(), FakeSmtp::start());
    // The newer Message is the selection, so a Compose is from its Account.
    let second = FakeImap::start(vec![FakeMessage::new(
        "Bob",
        "Bravo notes",
        "2024-11-19T09:00:00Z",
    )]);
    second.as_icloud();
    let server = |port: u16| format!("host = \"127.0.0.1\"\nport = {port}\ntls = false\n");
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\nsignature = \"First\"\n\n[imap]\n{}\n[smtp]\n{}\n\
         [secrets]\nstore = \"file\"\n{}\n\
         [[account]]\nemail = \"{OTHER}\"\nprovider = \"icloud\"\n\n[account.imap]\n{}",
        server(first.port),
        server(smtp.port),
        fake_oauth::shared().config(),
        server(second.port),
    ));
    let secret = home.secret_path_of(OTHER);
    fs::write(&secret, format!("{APP_PASSWORD}\n")).unwrap();
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
    let editor = Editor::new();
    let mut t = postal(&home, &editor.command());
    let both = |s: &Screen| listed(s) && s.contains("Bravo notes") && !s.contains("Secrets");
    t.wait_until(both).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(previewed).unwrap();
    assert_eq!(editor.opened(), "To: \nCc: \nSubject: \n\n");

    t.send(Key::Char('q')).unwrap();
    t.wait_until(both).unwrap();
    t.send(Key::Char(',')).unwrap();
    t.wait_until(|s| s.contains("Secrets") && s.contains("First"))
        .unwrap();
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("Zoë Example")).unwrap();
    let config = fs::read_to_string(home.config_path()).unwrap();
    let config: toml::Table = config.parse().unwrap();
    assert_eq!(config["signature"].as_str(), Some("First"));
    assert_eq!(
        config["account"][0]["signature"].as_str(),
        Some("Zoë Example\nPostal")
    );

    t.send(Key::Char('q')).unwrap();
    t.wait_until(both).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(previewed).unwrap();
    quit(t);
    assert_eq!(
        editor.opened(),
        "To: \nCc: \nSubject: \n\n\n-- \nZoë Example\nPostal\n"
    );
}

#[test]
fn a_signature_given_twice_names_the_path_and_line() {
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\nsignature = \"Zoë\"\nsignature-file = \"signature.txt\"\n"
    ));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_postal"))
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let at = format!("{}:3:18:", home.config_path().display());
    assert!(stderr.contains(&at), "{stderr}");
    assert!(stderr.contains("keep one"), "{stderr}");
}

/// Whether the last `needle` on screen, which is the Preview's, is dimmed.
#[track_caller]
fn dimmed(screen: &Screen, needle: &str) -> bool {
    let found = (0..screen.rows()).rev().find_map(|row| {
        let text = screen.row_text(row);
        let at = text.rfind(needle)?;
        let col = text.get(..at)?.chars().count();
        Some((row, u16::try_from(col).unwrap()))
    });
    let (row, col) = found.unwrap_or_else(|| panic!("no {needle:?} on {screen}"));
    screen.cell(row, col).unwrap().style().dim
}

/// The Preview of the one Message `received`, once it shows `until`.
fn previewing(received: FakeMessage, keys: &[Key], until: &str) -> Screen {
    let imap = FakeImap::start(vec![received]);
    let home = Home::for_server(&imap);
    let mut t = postal(&home, "true");
    t.wait_until(listed).unwrap();
    for key in keys {
        t.send(*key).unwrap();
    }

    let screen = t.snapshot_after(|s| s.contains(until)).unwrap();
    quit(t);
    screen
}

fn weekly_notes(body: &str) -> FakeMessage {
    FakeMessage::new("Ada Lovelace", "Weekly notes", "2024-11-18T09:00:00Z")
        .body(FakeBody::plain(body))
}

#[test]
fn a_received_signature_shows_dimmed() {
    let body = "See you there\r\n\r\n-- \r\nCountess of Engines\r\nLondon office\r\n";
    let screen = previewing(weekly_notes(body), &[], "London office");

    assert!(!dimmed(&screen, "See you there"), "{screen}");
    assert!(dimmed(&screen, "Countess of Engines"), "{screen}");
    assert!(dimmed(&screen, "London office"), "{screen}");
}

// Two hyphens alone rule off a paragraph in many a Message.
#[test]
fn a_separator_without_its_space_starts_no_signature() {
    let body = "See you there\r\n\r\n--\r\nSecond thoughts\r\n";
    let screen = previewing(weekly_notes(body), &[], "Second thoughts");

    assert!(!dimmed(&screen, "Second thoughts"), "{screen}");
}

// Postal's own Replies sign above the Quote, which is no part of it.
#[test]
fn a_signature_ends_where_the_quote_starts() {
    let body = "See you there\r\n\r\n-- \r\nCountess of Engines\r\n\r\n\
        On Mon, 4 Mar 2024 at 09:00, Bob <bob@example.com> wrote:\r\n> Where to?\r\n";
    let reply = weekly_notes(body).header("In-Reply-To", "<plans@example.com>");
    let screen = previewing(reply.clone(), &[], "quoted text hidden");
    assert!(dimmed(&screen, "Countess of Engines"), "{screen}");

    let screen = previewing(reply, &[Key::Char('z')], "> Where to?");
    assert!(dimmed(&screen, "Countess of Engines"), "{screen}");
    assert!(!dimmed(&screen, "> Where to?"), "{screen}");
    assert!(!dimmed(&screen, "wrote:"), "{screen}");
}

/// Settings, once the Message List shows; the selection is on the Account.
fn settings(t: &mut Terminal) {
    t.wait_until(listed).unwrap();
    t.send(Key::Char(',')).unwrap();
    t.wait_until(|s| s.contains("Settings") && s.contains("Secrets"))
        .unwrap();
}

/// The signature `config.toml` gives the Account as text.
fn configured(home: &Home) -> Option<String> {
    let config = fs::read_to_string(home.config_path()).unwrap();
    let config: toml::Table = config.parse().unwrap();
    config.get("signature")?.as_str().map(str::to_owned)
}

// Settings open on the Accounts read at start: the next Compose signs
// with what was typed there.
#[test]
fn enter_on_an_account_in_settings_edits_its_signature() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp, "");
    let editor = Editor::new();
    let mut t = postal(&home, &editor.command());
    settings(&mut t);
    t.send(Key::Enter).unwrap();
    // The Account's row shows how it signs now.
    t.wait_until(|s| s.contains("Zoë Example")).unwrap();
    assert_eq!(configured(&home).as_deref(), Some("Zoë Example\nPostal"));
    assert_eq!(editor.opened(), "");

    t.send(Key::Char('q')).unwrap();
    t.wait_until(|s| listed(s) && !s.contains("Secrets"))
        .unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(previewed).unwrap();
    quit(t);
    assert_eq!(
        editor.opened(),
        "To: \nCc: \nSubject: \n\n\n-- \nZoë Example\nPostal\n"
    );
}

#[test]
fn a_signature_emptied_in_settings_leaves_the_config() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp, "signature = \"Old\"\n");
    let editor = Editor::new();
    fs::write(editor.dir.path().join("typed"), "\n").unwrap();
    let mut t = postal(&home, &editor.command());
    settings(&mut t);
    assert!(t.screen().contains("Old"));
    t.send(Key::Enter).unwrap();
    // Settings again, not the screen the editor was handed.
    t.wait_until(|s| s.contains("Secrets") && !s.contains("Old"))
        .unwrap();
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(!config.contains("signature"), "{config}");
    assert_eq!(editor.opened(), "Old\n");
    quit(t);
}

// The config names the file, so the file is what changes.
#[test]
fn settings_edit_a_signature_file_in_place() {
    let (imap, smtp) = (inbox(), FakeSmtp::start());
    let home = home(&imap, &smtp, "signature-file = \"signature.txt\"\n");
    let file = home.config_path().with_file_name("signature.txt");
    fs::write(&file, "Old\n").unwrap();
    let before = fs::read_to_string(home.config_path()).unwrap();
    let editor = Editor::new();
    let mut t = postal(&home, &editor.command());
    settings(&mut t);
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("Zoë Example")).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), TYPED);
    assert_eq!(editor.opened(), "Old\n");
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), before);
    quit(t);
}
