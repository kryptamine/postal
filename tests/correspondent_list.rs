//! `postal correspondent list` prints every Correspondent the Cache knows,
//! ranked, for an editor to complete a Draft's recipients from: learnt
//! from synced mail, the Folder with the Sent role, every envelope of All
//! Mail or else the Inbox, and the Outbox. It only reads the Cache a TUI
//! synced. See ADR 0016 and ADR 0017.

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
    path::PathBuf,
    process::Output,
    time::{Duration, Instant},
};

use serde_json::Value;
use support::{
    fake_imap::{APP_PASSWORD, EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
};
use tempfile::TempDir;
use termlens::{Key, Terminal};

const SENT: &str = "[Gmail]/Sent Mail";
const ALL_MAIL: &str = "[Gmail]/All Mail";

/// The second Account, at iCloud.
const OTHER: &str = "me@icloud.example";

fn server(port: u16) -> String {
    format!("host = \"127.0.0.1\"\nport = {port}\ntls = false\n")
}

fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\n{}\n[smtp]\n{}\n[secrets]\nstore = \"file\"\n{}",
        server(imap.port),
        server(smtp.port),
        fake_oauth::shared().config()
    ))
}

/// The TUI, with `$EDITOR` copying `draft` over each Draft.
fn postal(home: &Home, draft: &str) -> (Terminal, TempDir) {
    let editor = tempfile::tempdir().unwrap();
    let next = editor.path().join("next");
    fs::write(&next, draft).unwrap();
    let t = Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("EDITOR", format!("cp '{}'", next.display()))
        .env("TZ", "UTC")
        .env("POSTAL_SEND_DELAY", "0")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    (t, editor)
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// `postal correspondent` with `args`.
fn list(home: &Home, args: &[&str]) -> Output {
    home.command(env!("CARGO_BIN_EXE_postal"))
        .args(["correspondent", "list"])
        .args(args)
        .output()
        .unwrap()
}

/// The `--json` answer to `args`, as `(address, name, written)` best first.
fn listed(home: &Home, args: &[&str]) -> Vec<(String, Option<String>, bool)> {
    let mut args = args.to_vec();
    args.push("--json");
    let output = list(home, &args);
    assert!(output.status.success(), "{output:?}");
    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    rows.iter()
        .map(|row| {
            (
                row["address"].as_str().unwrap().to_owned(),
                row["name"].as_str().map(str::to_owned),
                row["written"].as_bool().unwrap(),
            )
        })
        .collect()
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

fn from(name: &'static str, address: &'static str, subject: &str, date: &str) -> FakeMessage {
    FakeMessage {
        from_address: address,
        ..FakeMessage::new(name, subject, date)
    }
}

fn log_says(home: &Home, text: &str) -> bool {
    fs::read_to_string(home.log_path()).is_ok_and(|log| log.contains(text))
}

/// Every address `postal correspondent list` answers, best first.
fn addresses(home: &Home) -> Vec<String> {
    listed(home, &[]).into_iter().map(|(a, _, _)| a).collect()
}

#[test]
fn whom_the_account_wrote_to_in_sent_ranks_first_and_no_one_unanswerable_is_listed() {
    let imap = FakeImap::start(vec![
        from(
            "Petra Svoboda",
            "petra@example.com",
            "Notes",
            "2024-11-18T09:00:00Z",
        ),
        from(
            "GitHub",
            "noreply@github.com",
            "Build",
            "2024-11-17T09:00:00Z",
        ),
        FakeMessage::new("Me", "Self", "2024-11-16T09:00:00Z").sent_by_me(),
    ]);
    let sent = FakeMessage::new("Me", "Plans", "2024-01-02T09:00:00Z")
        .sent_by_me()
        .to("Petr Novak", "petr@example.com")
        .cc("ada@example.com", "ada@example.com")
        .message_id("<plans@example.com>");
    imap.add_folder(SENT, &["\\Sent"], vec![sent]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let (mut t, _editor) = postal(&home, "");
    t.wait_until(|s| s.contains("Notes")).unwrap();
    // The pass over Sent waits for the fetch connection to go quiet.
    wait_for(|| log_says(&home, &format!("from 1 Messages of {SENT}")));

    // A TUI open on the same Cache is no obstacle.
    let rows = listed(&home, &[]);
    quit(t);
    assert_eq!(
        rows,
        [
            ("ada@example.com".to_owned(), None, true),
            (
                "petr@example.com".to_owned(),
                Some("Petr Novak".to_owned()),
                true
            ),
            (
                "petra@example.com".to_owned(),
                Some("Petra Svoboda".to_owned()),
                false
            ),
        ]
    );

    let table = String::from_utf8(list(&home, &[]).stdout).unwrap();
    assert_eq!(
        table,
        "ada@example.com\nPetr Novak <petr@example.com>\nPetra Svoboda <petra@example.com>\n"
    );
}

#[test]
fn whom_a_send_went_to_is_listed_at_once() {
    let imap = FakeImap::start(vec![from(
        "Ada Lovelace",
        "ada@example.com",
        "Notes",
        "2024-11-18T09:00:00Z",
    )]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let (mut t, _editor) = postal(
        &home,
        "To: Зоя Петрова <zoya@example.com>\nSubject: Hi\n\nHello\n",
    );
    t.wait_until(|s| s.contains("Notes")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains(" Draft ")).unwrap();
    t.send(Key::Char('y')).unwrap();
    wait_for(|| smtp.received().len() == 1);
    wait_for(|| log_says(&home, "SMTP accepted"));
    quit(t);

    let rows = listed(&home, &[]);
    assert_eq!(
        rows[0],
        (
            "zoya@example.com".to_owned(),
            Some("Зоя Петрова".to_owned()),
            true
        )
    );
}

#[test]
fn a_drafts_account_ranks_its_correspondents_first() {
    let first = FakeImap::start(vec![from(
        "Alice",
        "alice@example.com",
        "Alpha",
        "2024-11-18T09:00:00Z",
    )]);
    let second = FakeImap::start(vec![from(
        "Bob",
        "bob@example.com",
        "Bravo",
        "2024-11-17T09:00:00Z",
    )]);
    second.as_icloud();
    let smtp = FakeSmtp::start();
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\n{}\n[smtp]\n{}\n[secrets]\nstore = \"file\"\n{}\n\
         [[account]]\nemail = \"{OTHER}\"\nprovider = \"icloud\"\n\n\
         [account.imap]\n{}\n[account.smtp]\n{}",
        server(first.port),
        server(smtp.port),
        fake_oauth::shared().config(),
        server(second.port),
        server(smtp.port),
    ));
    let secret = home.secret_path_of(OTHER);
    fs::write(&secret, format!("{APP_PASSWORD}\n")).unwrap();
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
    let (mut t, _editor) = postal(&home, "");
    t.wait_until(|s| s.contains("Alpha") && s.contains("Bravo"))
        .unwrap();
    quit(t);

    let addresses = |args: &[&str]| -> Vec<String> {
        listed(&home, args).into_iter().map(|(a, _, _)| a).collect()
    };

    assert_eq!(addresses(&[]), ["alice@example.com", "bob@example.com"]);
    assert_eq!(
        addresses(&["--account", OTHER]),
        ["bob@example.com", "alice@example.com"]
    );

    // A Draft names its Account in the notes beside it.
    let drafts = home.drafts_dir();
    fs::create_dir_all(&drafts).unwrap();
    let draft: PathBuf = drafts.join("1.eml");
    fs::write(&draft, "To: \nSubject: \n\n").unwrap();
    fs::write(
        drafts.join("1.json"),
        format!("{{\"account\":\"{OTHER}\"}}"),
    )
    .unwrap();
    assert_eq!(
        addresses(&["--draft", draft.to_str().unwrap()]),
        ["bob@example.com", "alice@example.com"]
    );

    // With several Accounts, the table names the one that knows each.
    let table = String::from_utf8(list(&home, &[]).stdout).unwrap();
    assert!(
        table.starts_with(&format!("Alice <alice@example.com>  {EMAIL}\n")),
        "{table}"
    );
}

#[test]
fn without_a_cache_it_fails_and_creates_nothing() {
    let (imap, smtp) = (FakeImap::start(Vec::new()), FakeSmtp::start());
    let home = home(&imap, &smtp);
    let output = list(&home, &["--json"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr, "postal: Cache not ready: run `postal` once\n");
    assert!(!home.cache_path().exists());
    // The TUI's log is left alone.
    assert!(!home.log_path().exists());
    assert_eq!(imap.commands(), Vec::<String>::new());
}

#[test]
fn mail_in_sent_from_an_alias_counts_as_written_and_the_alias_is_no_one() {
    let imap = FakeImap::start(vec![from(
        "Ivan Petrov",
        "ivan@example.com",
        "Notes",
        "2024-11-18T09:00:00Z",
    )]);
    let sent = from("Me", "alias@example.com", "Plans", "2024-01-02T09:00:00Z")
        .to("Ivan Petrov", "ivan@example.com")
        .message_id("<plans@example.com>");
    imap.add_folder(SENT, &["\\Sent"], vec![sent]);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let (mut t, _editor) = postal(&home, "");
    t.wait_until(|s| s.contains("Notes")).unwrap();
    wait_for(|| log_says(&home, &format!("from 1 Messages of {SENT}")));
    quit(t);

    assert_eq!(
        listed(&home, &[]),
        [(
            "ivan@example.com".to_owned(),
            Some("Ivan Petrov".to_owned()),
            true
        )]
    );
}

#[test]
fn reading_a_thread_learns_whom_all_mail_holds_in_it() {
    let imap = FakeImap::start(vec![
        from("Bob", "bob@example.com", "Trip", "2024-03-03T09:00:00Z").in_thread(0),
    ]);
    // Dora's answer sits under a label never shown.
    imap.add_folder(
        "Work",
        &[],
        vec![
            from(
                "Dora",
                "dora@example.com",
                "Re: Trip",
                "2024-03-04T09:00:00Z",
            )
            .in_thread(0),
        ],
    );
    imap.add_all_mail("[Gmail]/All Mail");
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let (mut t, _editor) = postal(&home, "");
    t.wait_until(|s| s.contains("Trip")).unwrap();
    t.send(Key::Enter).unwrap();
    wait_for(|| log_says(&home, "holds 2 Messages in All Mail"));
    quit(t);

    assert_eq!(addresses(&home), ["dora@example.com", "bob@example.com"]);
}

#[test]
fn every_message_in_all_mail_is_learnt_and_then_only_what_arrives() {
    let imap = FakeImap::start(vec![from(
        "Bob",
        "bob@example.com",
        "Trip",
        "2024-03-03T09:00:00Z",
    )]);
    // Never shown: Dora's label is learnt from All Mail, and Trash, which
    // All Mail leaves out, is not.
    let dora = from("Dora", "dora@example.com", "Plans", "2024-03-01T09:00:00Z");
    imap.add_folder("Work", &[], vec![dora]);
    let tom = from("Tom", "tom@example.com", "Junk", "2024-03-02T09:00:00Z");
    imap.add_folder("[Gmail]/Trash", &["\\Trash"], vec![tom]);
    imap.add_all_mail(ALL_MAIL);
    let smtp = FakeSmtp::start();
    let home = home(&imap, &smtp);
    let (mut t, _editor) = postal(&home, "");
    t.wait_until(|s| s.contains("Trip")).unwrap();
    wait_for(|| log_says(&home, &format!("read every envelope of {ALL_MAIL}")));
    quit(t);
    assert_eq!(addresses(&home), ["bob@example.com", "dora@example.com"]);

    let erin = from("Erin", "erin@example.com", "Later", "2024-03-04T09:00:00Z");
    imap.deliver_to("Work", erin);
    let (mut t, _editor) = postal(&home, "");
    t.wait_until(|s| s.contains("Trip")).unwrap();
    // The second connection reads past the last UID the first one read.
    wait_for(|| log_says(&home, &format!("from 1 Messages of {ALL_MAIL}")));
    quit(t);
    assert_eq!(
        addresses(&home),
        ["erin@example.com", "bob@example.com", "dora@example.com"]
    );
}

#[test]
fn off_gmail_the_inbox_is_learnt_past_the_cached_window() {
    let olga = from("Olga", "olga@example.com", "Oldest", "2023-01-01T09:00:00Z");
    let newer = (0..500).map(|day| {
        let date = "2024-01-01T09:00:00Z"
            .parse::<chrono::DateTime<chrono::Utc>>()
            .unwrap()
            + chrono::Duration::days(day);
        FakeMessage::new("Newer", "Newer", &date.to_rfc3339())
    });
    let imap = FakeImap::start(std::iter::once(olga).chain(newer).collect());
    imap.as_icloud();
    let smtp = FakeSmtp::start();
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\nprovider = \"icloud\"\n\n[imap]\n{}\n[smtp]\n{}\n\
         [secrets]\nstore = \"file\"\n",
        server(imap.port),
        server(smtp.port),
    ));
    fs::write(home.secret_path(), format!("{APP_PASSWORD}\n")).unwrap();
    let (mut t, _editor) = postal(&home, "");
    t.wait_until(|s| s.contains("Newer")).unwrap();
    wait_for(|| log_says(&home, "read every envelope of INBOX"));
    quit(t);
    assert!(addresses(&home).contains(&"olga@example.com".to_owned()));
}
