//! Reply with `r` and Reply All with `R` open `$EDITOR` on a Draft
//! prefilled from the Message answered, which the editor sends as
//! `:PostalSend` does; once the fake SMTP server takes it, the original gets
//! `\Answered` on the fake IMAP server.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    fs,
    time::{Duration, Instant},
};

use mail_parser::{Address, MessageParser};
use support::{
    editor::{preview_newest, saved, sending},
    fake_body::FakeBody,
    fake_imap::{EMAIL, FIRST_UID, FakeImap, FakeMessage, INBOX, Stall},
    fake_oauth,
    fake_smtp::{FakeSmtp, Received},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

const DATE: &str = "2024-11-18T09:00:00Z";

/// How the quote's first line names `DATE` in UTC.
const SAID: &str = "On Mon, 18 Nov 2024 09:00, Ada Lovelace wrote:";

fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp.port,
        fake_oauth::shared().config()
    ))
}

/// `$EDITOR` runs `editor`, which leaves the Draft as Postal prefilled it.
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
        .env("POSTAL_SEND_DELAY", "0")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

/// Ada's Message, answered in the Thread before.
fn from_ada(body: FakeBody) -> FakeMessage {
    FakeMessage {
        from_address: "ada@example.com",
        ..FakeMessage::new("Ada Lovelace", "Weekly notes", DATE)
    }
    .header("To", &format!("Me <{EMAIL}>"))
    .header("Message-ID", "<notes-2@example.com>")
    .header("In-Reply-To", "<notes-1@example.com>")
    .header("References", "<notes-0@example.com> <notes-1@example.com>")
    .body(body)
    .in_thread(0)
}

fn plain() -> FakeBody {
    FakeBody::plain("Line one\r\n\r\n> quoted before\r\nLine two\r\n")
}

/// The Account's own answer, newer than Ada's in the same Thread.
fn mine() -> FakeMessage {
    FakeMessage::new("Me", "Re: Weekly notes", "2024-11-19T09:00:00Z")
        .sent_by_me()
        .body(FakeBody::plain("My answer"))
        .in_thread(0)
}

fn listed(screen: &Screen) -> bool {
    screen.contains("Weekly notes") && !screen.contains(" Draft ") && !local(screen)
}

fn local(screen: &Screen) -> bool {
    screen.contains(" Local Drafts ")
}

/// The editor that sends the Draft untouched.
fn sends() -> String {
    sending("true")
}

fn previewed(screen: &Screen) -> bool {
    screen.contains(" Draft ") && screen.contains("y send")
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

/// `keys` on the Message List, the last opening the editor that sends
/// their Draft: what the server received.
fn replied(home: &Home, smtp: &FakeSmtp, keys: &[Key]) -> Received {
    let mut t = postal(home, &sends());
    t.wait_until(listed).unwrap();
    press(&mut t, keys);
    t.wait_until(|s| s.contains("Sent") && !s.contains(" Draft "))
        .unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let received = smtp.received();
    assert_eq!(received.len(), 1);
    received.into_iter().next().unwrap()
}

fn press(t: &mut Terminal, keys: &[Key]) {
    for key in keys {
        t.send(*key).unwrap();
    }
}

/// What each Draft's file holds, its notes left out.
fn drafts(home: &Home) -> Vec<String> {
    let Ok(entries) = fs::read_dir(home.drafts_dir()) else {
        return Vec::new();
    };

    entries
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "eml"))
        .map(|path| fs::read_to_string(path).unwrap())
        .collect()
}

fn mailboxes(address: Option<&Address<'_>>) -> Vec<String> {
    address.map_or_else(Vec::new, |address| {
        address
            .iter()
            .map(|a| a.address().unwrap().to_owned())
            .collect()
    })
}

/// Each line of the decoded text body, without CRLF.
fn body_lines(received: &Received) -> Vec<String> {
    let message = MessageParser::default()
        .parse(received.data.as_bytes())
        .unwrap();
    message
        .body_text(0)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_reply_from_the_list_answers_the_newest_message_not_from_the_account() {
    let imap = FakeImap::start(vec![
        from_ada(plain()).header("Reply-To", "\"Lovelace, Ada\" <notes@example.com>"),
        mine(),
    ]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let mut t = postal(&home, "true");
    t.wait_until(listed).unwrap();
    t.send(Key::Char('r')).unwrap();
    t.wait_until(saved).unwrap();

    // In-Reply-To and References are Postal's, and stay out of the file;
    // a name with a comma stays quoted, one address.
    let drafts = drafts(&home);
    assert_eq!(drafts.len(), 1);
    assert_eq!(
        drafts[0],
        format!(
            "To: \"Lovelace, Ada\" <notes@example.com>\nCc: \nSubject: Re: Weekly notes\n\n\n\
             {SAID}\n> Line one\n>\n>> quoted before\n> Line two\n"
        )
    );

    preview_newest(&mut t);
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Sent") && local(s)).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let received = &smtp.received()[0];
    assert_eq!(received.rcpt_to, ["notes@example.com"]);
    let message = MessageParser::default()
        .parse(received.data.as_bytes())
        .unwrap();
    assert_eq!(message.subject(), Some("Re: Weekly notes"));
    assert_eq!(
        message.in_reply_to().as_text(),
        Some("notes-2@example.com"),
        "{}",
        received.data
    );
    assert_eq!(
        message.references().as_text_list().unwrap(),
        [
            "notes-0@example.com",
            "notes-1@example.com",
            "notes-2@example.com"
        ]
    );
    assert_eq!(
        body_lines(received),
        [
            "",
            SAID,
            "> Line one",
            ">",
            ">> quoted before",
            "> Line two"
        ]
    );
}

// Reading answers the Message under the cursor, not the newest.
#[test]
fn a_reply_from_reading_answers_the_message_at_the_cursor() {
    let bob = FakeMessage {
        from_address: "bob@example.com",
        ..FakeMessage::new("Bob", "Re: Weekly notes", "2024-11-19T09:00:00Z")
    }
    .header("Message-ID", "<bob@example.com>")
    .in_thread(0);
    let imap = FakeImap::start(vec![from_ada(plain()), bob]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let received = replied(&home, &smtp, &[Key::Enter, Key::Char('p'), Key::Char('r')]);

    assert_eq!(received.rcpt_to, ["ada@example.com"]);
    let message = MessageParser::default()
        .parse(received.data.as_bytes())
        .unwrap();
    assert_eq!(message.in_reply_to().as_text(), Some("notes-2@example.com"));
}

#[test]
fn a_subject_answered_already_gets_no_second_re() {
    let bob = FakeMessage {
        from_address: "bob@example.com",
        ..FakeMessage::new("Bob", "RE: Weekly notes", DATE)
    };
    let imap = FakeImap::start(vec![bob.body(plain())]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let received = replied(&home, &smtp, &[Key::Char('r')]);
    let message = MessageParser::default()
        .parse(received.data.as_bytes())
        .unwrap();
    assert_eq!(message.subject(), Some("RE: Weekly notes"));
}

#[test]
fn reply_all_copies_everyone_else_but_the_account() {
    let ada = from_ada(plain())
        .header("To", "ME@example.com, Bob <bob@example.com>")
        .header(
            "Cc",
            "carol@example.com, Ada <ada@example.com>, bob@example.com",
        );
    let imap = FakeImap::start(vec![ada]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let received = replied(&home, &smtp, &[Key::Char('R')]);

    let message = MessageParser::default()
        .parse(received.data.as_bytes())
        .unwrap();
    assert_eq!(mailboxes(message.to()), ["ada@example.com"]);
    assert_eq!(
        mailboxes(message.cc()),
        ["bob@example.com", "carol@example.com"]
    );
    assert_eq!(
        received.rcpt_to,
        ["ada@example.com", "bob@example.com", "carol@example.com"]
    );
}

#[test]
fn reply_copies_no_one_else() {
    let ada = from_ada(plain()).header("Cc", "carol@example.com");
    let imap = FakeImap::start(vec![ada]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let received = replied(&home, &smtp, &[Key::Char('r')]);
    assert_eq!(received.rcpt_to, ["ada@example.com"]);
}

#[test]
fn a_reply_to_html_only_mail_quotes_its_text() {
    let html = FakeBody::text(
        "html",
        "7bit",
        "<html><head><style>p { color: red }</style></head><body>\
         <p>Hello <a href=\"https://example.com/x\">there</a></p><p>Second &amp; last</p>\
         </body></html>",
    );
    let imap = FakeImap::start(vec![from_ada(html)]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let received = replied(&home, &smtp, &[Key::Char('r')]);

    let lines = body_lines(&received);
    assert_eq!(lines[1], SAID);
    let quote: Vec<_> = lines[2..].iter().filter(|l| *l != ">").collect();
    assert_eq!(quote, ["> Hello there", "> Second & last"], "{lines:?}");
}

/// The rows of the Cache's body table.
fn bodies(home: &Home) -> Vec<i64> {
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let mut statement = cache.prepare("SELECT key FROM body ORDER BY key").unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn a_reply_to_a_message_whose_body_was_evicted_fetches_it_first() {
    let imap = FakeImap::start(vec![from_ada(plain()).unread(), mine()]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let ada = imap.msgid(FIRST_UID).cast_signed();

    // Reading unfolds the Unread Message, which fetches its body.
    let mut t = postal(&home, "true");
    t.wait_until(listed).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("Line one")).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert!(bodies(&home).contains(&ada));

    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    cache
        .execute("DELETE FROM body WHERE key = ?1", [ada])
        .unwrap();
    drop(cache);

    let mut t = postal(&home, "true");
    t.wait_until(|s| listed(s) && s.contains("My answer"))
        .unwrap();
    imap.clear_commands();
    imap.stall(Stall::Body);
    t.send(Key::Char('r')).unwrap();
    t.wait_until(|s| s.contains("fetching the Message…"))
        .unwrap();
    assert!(drafts(&home).is_empty(), "the editor waits for the body");
    wait_for(|| {
        imap.commands()
            .iter()
            .any(|c| c.starts_with(&format!("UID FETCH {FIRST_UID} (UID BODY.PEEK[HEADER]")))
    });

    imap.release();
    t.wait_until(saved).unwrap();
    assert!(!t.screen().contains("fetching the Message…"));
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert!(drafts(&home)[0].contains("> Line one"));
}

#[test]
fn a_sent_reply_flags_the_original_answered() {
    let imap = FakeImap::start(vec![from_ada(plain())]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let mut t = postal(&home, "true");
    t.wait_until(listed).unwrap();
    t.send(Key::Char('r')).unwrap();
    preview_newest(&mut t);
    assert!(!imap.answered(INBOX, FIRST_UID));

    // Discarded, then brought back, the Draft still answers the Message.
    t.send(Key::Char('d')).unwrap();
    t.wait_until(|s| s.contains("Discarded · u undo")).unwrap();
    t.send(Key::Char('u')).unwrap();
    t.wait_until(previewed).unwrap();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Sent") && local(s)).unwrap();
    wait_for(|| imap.answered(INBOX, FIRST_UID));
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let commands = imap.commands();
    assert!(
        commands
            .iter()
            .any(|c| c == &format!("UID STORE {FIRST_UID} +FLAGS.SILENT (\\Answered)")),
        "{commands:#?}"
    );
    let received = smtp.received();
    let message = MessageParser::default()
        .parse(received[0].data.as_bytes())
        .unwrap();
    assert_eq!(message.in_reply_to().as_text(), Some("notes-2@example.com"));
}

// Gmail files a sent Reply in Sent Mail, outside the shown Folder: without
// the Thread read again, Reading would not show it under what it answers.
#[test]
fn a_sent_reply_joins_the_stack_it_answers() {
    let imap = FakeImap::start(vec![from_ada(plain())]);
    imap.add_folder("[Gmail]/Sent Mail", &["\\Sent"], Vec::new());
    imap.add_all_mail("[Gmail]/All Mail");
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let mut t = postal(&home, &sends());
    t.wait_until(listed).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("Line one")).unwrap();
    wait_for(|| {
        imap.commands()
            .iter()
            .any(|c| c.starts_with("UID SEARCH X-GM-THRID"))
    });

    // The fake SMTP server files nothing: the copy Gmail keeps goes in ahead.
    imap.deliver_to("[Gmail]/Sent Mail", mine());
    imap.clear_commands();
    t.send(Key::Char('r')).unwrap();
    t.wait_until(|s| s.contains("My answer") && !s.contains(" Draft "))
        .unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    // Read before the server has it, the Thread would drop `\Answered`.
    let commands = imap.commands();
    let at = |wanted: &dyn Fn(&str) -> bool| commands.iter().position(|c| wanted(c));
    let store = at(&|c| c == format!("UID STORE {FIRST_UID} +FLAGS.SILENT (\\Answered)")).unwrap();
    let search = at(&|c| c.starts_with("UID SEARCH X-GM-THRID")).unwrap();
    assert!(store < search, "{commands:#?}");
}

// `\Answered` tells other clients the Message was answered: a Reply still
// in the Outbox has not been.
#[test]
fn a_reply_flags_the_original_answered_only_once_smtp_accepts_it() {
    let imap = FakeImap::start(vec![from_ada(plain())]);
    let smtp = FakeSmtp::start();
    smtp.down();
    let home = home(&imap, &smtp);
    let mut t = postal(&home, &sends());
    t.wait_until(listed).unwrap();
    t.send(Key::Char('r')).unwrap();
    t.wait_until(|s| listed(s) && s.contains("1 outbox"))
        .unwrap();
    wait_for(|| smtp.turned_away() > 0);
    assert!(!imap.answered(INBOX, FIRST_UID));

    smtp.up();
    imap.deliver(FakeMessage::new("Bob", "Lunch", "2024-11-19T09:00:00Z"));
    wait_for(|| imap.answered(INBOX, FIRST_UID));
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(smtp.received().len(), 1);
}
