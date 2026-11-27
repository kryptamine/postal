//! `postal thread show` prints the Thread a Reply's Draft answers, or a
//! Message `E` opened is of, oldest first, for an agent to read; it only
//! reads the Cache, so a Reply fetches its Thread's bodies first (ADR 0017).

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
    process::Output,
};

use serde_json::Value;
use support::{
    editor::saved,
    fake_body::FakeBody,
    fake_imap::{EMAIL, FIRST_UID, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
};
use termlens::{Key, Screen, Terminal};

fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    let server = |port| format!("host = \"127.0.0.1\"\nport = {port}\ntls = false\n");
    Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\n{}\n[smtp]\n{}\n[secrets]\nstore = \"file\"\n{}",
        server(imap.port),
        server(smtp.port),
        fake_oauth::shared().config()
    ))
}

/// `$EDITOR` is `true`: a Reply's Draft stays as Postal prefilled it.
fn postal(home: &Home) -> Terminal {
    postal_editing(home, "true")
}

fn postal_editing(home: &Home, editor: &str) -> Terminal {
    home.terminal()
        .size(100, 30)
        .env("EDITOR", editor)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

/// Each key once the screen shows its text, the last opening a Reply: the
/// path of its Draft, left unsent.
fn reply(home: &Home, keys: &[(&str, Key)]) -> PathBuf {
    let mut t = postal(home);
    for &(shown, key) in keys {
        t.wait_until(|s| s.contains(shown)).unwrap();
        t.send(key).unwrap();
    }

    t.wait_until(saved).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let mut drafts: Vec<_> = fs::read_dir(home.drafts_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "eml"))
        .collect();
    assert_eq!(drafts.len(), 1);
    drafts.pop().unwrap()
}

/// Whether the Preview, right of the Message List, shows `text`.
fn in_preview(screen: &Screen, text: &str) -> bool {
    (0..screen.rows()).any(|row| {
        let line = screen.row_text(row);
        line.split_once("││")
            .is_some_and(|(_, preview)| preview.contains(text))
    })
}

/// The path `$EDITOR` was handed after `E` on the Message whose Preview
/// shows `shown`.
fn opened(home: &Home, shown: &str) -> PathBuf {
    let record = home.xdg_cache_home().join("opened");
    let editor = format!("sh -c 'printf %s \"$1\" > {}' sh", record.display());
    let mut t = postal_editing(home, &editor);
    // The card's Snippet shows it before the body is in: `E` is then refused.
    t.wait_until(|s| in_preview(s, shown)).unwrap();
    t.send(Key::Char('E')).unwrap();
    t.wait_until(|_| record.exists()).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    PathBuf::from(fs::read_to_string(&record).unwrap())
}

fn show(home: &Home, args: &[&str]) -> Output {
    home.command(env!("CARGO_BIN_EXE_postal"))
        .args(["thread", "show"])
        .args(args)
        .output()
        .unwrap()
}

/// The `--json` answer for `draft`, as `(from, recipients, date, body)`.
fn shown(home: &Home, draft: &Path) -> Vec<(String, String, String, Option<String>)> {
    let output = show(home, &["--draft", draft.to_str().unwrap(), "--json"]);
    assert!(output.status.success(), "{output:?}");
    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    rows.iter()
        .map(|row| {
            (
                row["from"].as_str().unwrap().to_owned(),
                row["recipients"].as_str().unwrap().to_owned(),
                row["date"].as_str().unwrap().to_owned(),
                row["body"].as_str().map(|b| b.trim_end().to_owned()),
            )
        })
        .collect()
}

fn from_ada(subject: &str, date: &str, body: &str) -> FakeMessage {
    FakeMessage {
        from_address: "ada@example.com",
        ..FakeMessage::new("Ada Lovelace", subject, date)
    }
    .to("Me", EMAIL)
    .body(FakeBody::plain(body))
    .in_thread(0)
}

fn mine() -> FakeMessage {
    FakeMessage::new("Me", "Re: Weekly notes", "2024-11-19T09:00:00Z")
        .sent_by_me()
        .to("Ada Lovelace", "ada@example.com")
        .body(FakeBody::plain("My answer"))
        .in_thread(0)
}

#[test]
fn a_reply_fetches_every_body_of_its_thread_and_shows_them_oldest_first() {
    // Only the Thread's newest body is in the Preview; the oldest is asked
    // for by the Reply alone.
    let imap = FakeImap::start(vec![
        from_ada("Weekly notes", "2024-11-17T09:00:00Z", "First notes"),
        from_ada("Re: Weekly notes", "2024-11-18T09:00:00Z", "Second notes"),
        mine(),
    ]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let draft = reply(&home, &[("My answer", Key::Char('r'))]);

    let ada = "Ada Lovelace <ada@example.com>".to_owned();
    let me = format!("Me <{EMAIL}>");
    assert_eq!(
        shown(&home, &draft),
        [
            (
                ada.clone(),
                me.clone(),
                "2024-11-17T09:00:00+00:00".to_owned(),
                Some("First notes".to_owned())
            ),
            (
                ada.clone(),
                me.clone(),
                "2024-11-18T09:00:00+00:00".to_owned(),
                Some("Second notes".to_owned())
            ),
            (
                me,
                ada,
                "2024-11-19T09:00:00+00:00".to_owned(),
                Some("My answer".to_owned())
            ),
        ]
    );

    let text =
        String::from_utf8(show(&home, &["--draft", draft.to_str().unwrap()]).stdout).unwrap();
    assert!(
        text.starts_with(&format!(
            "From: Ada Lovelace <ada@example.com>\nRecipients: Me <{EMAIL}>\n\
             Date: Sun, 17 Nov 2024 09:00\n\nFirst notes\n\n\
             From: Ada Lovelace <ada@example.com>\n"
        )),
        "{text}"
    );
}

// The file is gone once the editor exits: its name alone names the Message.
#[test]
fn a_message_opened_in_the_editor_shows_its_whole_thread() {
    let imap = FakeImap::start(vec![
        from_ada("Weekly notes", "2024-11-17T09:00:00Z", "First notes"),
        from_ada("Re: Weekly notes", "2024-11-18T09:00:00Z", "Second notes"),
        mine(),
    ]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let message = opened(&home, "My answer");

    let output = show(&home, &["--message", message.to_str().unwrap(), "--json"]);
    assert!(output.status.success(), "{output:?}");
    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    let dates: Vec<_> = rows
        .iter()
        .map(|row| row["date"].as_str().unwrap())
        .collect();
    assert_eq!(
        dates,
        [
            "2024-11-17T09:00:00+00:00",
            "2024-11-18T09:00:00+00:00",
            "2024-11-19T09:00:00+00:00"
        ]
    );

    let elsewhere = format!(
        "/tmp/other@example.com/{}",
        message.file_name().unwrap().display()
    );
    let output = show(&home, &["--message", &elsewhere]);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim_end(),
        "postal: other@example.com is no Account set up"
    );
}

#[test]
fn a_body_missing_from_the_cache_is_marked_not_skipped() {
    let imap = FakeImap::start(vec![
        from_ada("Weekly notes", "2024-11-18T09:00:00Z", "Notes"),
        mine(),
    ]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let draft = reply(&home, &[("My answer", Key::Char('r'))]);

    let mine = imap.msgid(FIRST_UID + 1).cast_signed();
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    cache
        .execute("DELETE FROM body WHERE key = ?1", [mine])
        .unwrap();
    drop(cache);

    let bodies: Vec<_> = shown(&home, &draft).into_iter().map(|row| row.3).collect();
    assert_eq!(bodies, [Some("Notes".to_owned()), None]);
    let text =
        String::from_utf8(show(&home, &["--draft", draft.to_str().unwrap()]).stdout).unwrap();
    assert!(text.ends_with("\n\n[body not in the Cache]\n"), "{text}");
}

#[test]
fn a_reply_to_a_message_found_only_with_its_thread_shows_that_thread() {
    let imap = FakeImap::start(vec![from_ada(
        "Trip",
        "2024-03-03T09:00:00Z",
        "Shall we go?",
    )]);
    // Dora's answer sits under a label never shown.
    let dora = FakeMessage {
        from_address: "dora@example.com",
        ..FakeMessage::new("Dora", "Re: Trip", "2024-03-04T09:00:00Z")
    }
    .to("Me", EMAIL)
    .body(FakeBody::plain("Yes"))
    .in_thread(0);
    imap.add_folder("Work", &[], vec![dora]);
    imap.add_all_mail("[Gmail]/All Mail");
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    // Dora's joins the stack below Ada's, where Reading opened.
    let draft = reply(
        &home,
        &[
            ("Trip", Key::Enter),
            ("Dora", Key::Char('n')),
            ("Dora", Key::Char('r')),
        ],
    );

    let senders: Vec<_> = shown(&home, &draft).into_iter().map(|row| row.0).collect();
    assert_eq!(
        senders,
        ["Ada Lovelace <ada@example.com>", "Dora <dora@example.com>"]
    );
}

#[test]
fn without_gmail_a_message_is_a_thread_of_its_own() {
    let imap = FakeImap::start(vec![
        from_ada("Weekly notes", "2024-11-18T09:00:00Z", "Notes"),
        mine(),
    ]);
    imap.without_gmail();
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let draft = reply(
        &home,
        &[("My answer", Key::Char('j')), ("My answer", Key::Char('r'))],
    );

    let bodies: Vec<_> = shown(&home, &draft).into_iter().map(|row| row.3).collect();
    assert_eq!(bodies, [Some("Notes".to_owned())]);
}

// A new mail is no error: the editor asks for every Draft's Thread.
#[test]
fn a_draft_answering_nothing_has_an_empty_thread() {
    let (imap, smtp) = (FakeImap::start(Vec::new()), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Inbox")).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let drafts = home.drafts_dir();
    fs::create_dir_all(&drafts).unwrap();
    let draft = drafts.join("1.eml");
    fs::write(&draft, "To: \nSubject: \n\n").unwrap();
    let output = show(&home, &["--draft", draft.to_str().unwrap()]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "");
    assert_eq!(String::from_utf8(output.stderr).unwrap(), "");
}

#[test]
fn a_thread_body_the_server_refuses_holds_no_reply_back() {
    let imap = FakeImap::start(vec![
        from_ada("Weekly notes", "2024-11-17T09:00:00Z", "First notes"),
        from_ada("Re: Weekly notes", "2024-11-18T09:00:00Z", "Second notes"),
        mine(),
    ]);
    imap.refuse_body(FIRST_UID);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let draft = reply(&home, &[("My answer", Key::Char('r'))]);

    let bodies: Vec<_> = shown(&home, &draft).into_iter().map(|row| row.3).collect();
    assert_eq!(
        bodies,
        [
            None,
            Some("Second notes".to_owned()),
            Some("My answer".to_owned())
        ]
    );
}
