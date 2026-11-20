//! Several Accounts in one run: each syncs on its own connections, the
//! Folder Picker lists a section per Account, and what is done to a
//! Message reaches the server of the Account it belongs to.

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
    time::{Duration, Instant},
};

use support::{
    fake_body::FakeBody,
    fake_imap::{APP_PASSWORD, EMAIL, FIRST_UID, FakeImap, FakeMessage, Outage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
    picker::{picked, picker},
};
use tempfile::TempDir;
use termlens::{Key, Screen, Terminal};

/// The second Account, at iCloud.
const OTHER: &str = "me@icloud.example";

const WRITTEN: &str = "To: Bob Example <bob@example.com>\nSubject: Plans\n\nHi Bob\n";

/// The first Account at a Gmail-like server, the second at an iCloud-like
/// one, each sending through its own SMTP server.
struct Two {
    first: FakeImap,
    second: FakeImap,
    /// The UID of the second Account's one Message.
    bravo: u32,
    smtp: (FakeSmtp, FakeSmtp),
    home: Home,
    editor: TempDir,
}

fn two() -> Two {
    let first = vec![
        FakeMessage::new("Alice", "Alpha news", "2024-03-02T09:00:00Z")
            .body(FakeBody::plain("Body of Alpha news")),
    ];
    let second = vec![
        FakeMessage::new("Bob", "Bravo notes", "2024-03-01T09:00:00Z")
            .body(FakeBody::plain("Body of Bravo notes"))
            .unread(),
    ];
    two_with(first, second)
}

/// As `two`, with `first` and `second` in each Account's Inbox; `bravo` is
/// the UID of the second's last.
fn two_with(first: Vec<FakeMessage>, second: Vec<FakeMessage>) -> Two {
    let first = FakeImap::start(first);
    let bravo = FIRST_UID + u32::try_from(second.len()).unwrap() - 1;
    let second = FakeImap::start(second);
    second.as_icloud();
    let smtp = (FakeSmtp::start(), FakeSmtp::start());
    let server = |port: u16| format!("host = \"127.0.0.1\"\nport = {port}\ntls = false\n");
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\n{}\n[smtp]\n{}\n[secrets]\nstore = \"file\"\n{}\n\
         [[account]]\nemail = \"{OTHER}\"\nprovider = \"icloud\"\n\n\
         [account.imap]\n{}\n[account.smtp]\n{}",
        server(first.port),
        server(smtp.0.port),
        fake_oauth::shared().config(),
        server(second.port),
        server(smtp.1.port),
    ));
    let secret = home.secret_path_of(OTHER);
    fs::write(&secret, format!("{APP_PASSWORD}\n")).unwrap();
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
    let editor = tempfile::tempdir().unwrap();
    fs::write(editor.path().join("next"), WRITTEN).unwrap();
    Two {
        first,
        second,
        bravo,
        smtp,
        home,
        editor,
    }
}

fn postal(two: &Two) -> Terminal {
    let editor = format!("cp '{}'", two.editor.path().join("next").display());
    Terminal::builder()
        .size(120, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", two.home.xdg_config_home())
        .env("XDG_STATE_HOME", two.home.xdg_state_home())
        .env("XDG_CACHE_HOME", two.home.xdg_cache_home())
        .env("XDG_DATA_HOME", two.home.xdg_data_home())
        .env("EDITOR", editor)
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
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

/// The Folder Picker's rows once both Accounts have listed their Folders.
fn both_listed(screen: &Screen) -> bool {
    let rows = picked(screen);
    rows.iter().any(|r| r == OTHER) && rows.iter().any(|r| r.starts_with("Trash"))
}

/// Shows the second Account's Inbox: the Query leaves All Inboxes and
/// each Account's Inbox, the second's last.
fn second_inbox(t: &mut Terminal) {
    t.wait_until(|s| s.contains("Alpha news")).unwrap();
    t.send(Key::Char('f')).unwrap();
    t.wait_until(both_listed).unwrap();
    t.send_str("inbox").unwrap();
    t.wait_until(|s| picked(s).len() == 5).unwrap();
    t.send(Key::Down).unwrap();
    t.send(Key::Down).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| {
        picker(s).is_empty() && s.contains("Bravo notes") && !s.contains("Alpha news")
    })
    .unwrap();
}

#[test]
fn the_picker_lists_all_inboxes_first_then_a_section_per_account_and_local_drafts_last() {
    let two = two();
    two.first
        .deliver(FakeMessage::new("Carol", "Charlie plans", "2024-03-03T09:00:00Z").unread());
    let mut t = postal(&two);
    t.wait_until(|s| s.contains("Charlie plans")).unwrap();
    t.send(Key::Char('f')).unwrap();
    // Its Unread count sums the Inboxes'.
    let screen = t
        .snapshot_after(|s| both_listed(s) && picked(s).iter().any(|r| r == "All Inboxes 2"))
        .unwrap();
    let rows = picked(&screen);
    let at = |row: &str| rows.iter().position(|r| r == row).unwrap();
    assert_eq!(rows[0], "All Inboxes 2", "{rows:?}");
    assert_eq!(rows[1], EMAIL, "{rows:?}");
    assert!(rows[2].starts_with("Inbox"), "{rows:?}");
    assert!(at(OTHER) > 2, "{rows:?}");
    assert!(rows[at(OTHER) + 1].starts_with("Inbox"), "{rows:?}");
    assert_eq!(rows.last().map(String::as_str), Some("Local Drafts"));

    // Typing narrows every section, and each keeps its heading.
    t.send_str("inb").unwrap();
    let screen = t.snapshot_after(|s| picked(s).len() == 5).unwrap();
    let rows = picked(&screen);
    assert_eq!([&*rows[1], &*rows[3]], [EMAIL, OTHER], "{rows:?}");
    assert!(rows[2].starts_with("Inbox") && rows[4].starts_with("Inbox"));
    quit(t);
}

#[test]
fn every_run_starts_in_all_inboxes_with_each_accounts_rows_by_arrival() {
    let two = two();
    let mut t = postal(&two);
    let screen = t
        .snapshot_after(|s| s.contains("Alpha news") && s.contains("Bravo notes"))
        .unwrap();
    assert!(screen.row_text(0).contains(" All Inboxes "), "{screen:?}");
    let row = |subject: &str| {
        (0..screen.rows())
            .find(|&r| screen.row_text(r).contains(subject))
            .unwrap()
    };
    assert!(row("Alpha news") < row("Bravo notes"), "{screen:?}");
    quit(t);
}

// Each Account idles on its own Inbox, so mail to either shows at once.
#[test]
fn all_inboxes_shows_new_mail_to_each_account_live() {
    let two = two();
    let mut t = postal(&two);
    t.wait_until(|s| s.contains("Alpha news") && s.contains("Bravo notes"))
        .unwrap();
    wait_for(|| two.first.commands().iter().any(|c| c == "IDLE"));
    wait_for(|| two.second.commands().iter().any(|c| c == "IDLE"));
    two.second.deliver(FakeMessage::new(
        "Dave",
        "Delta report",
        "2024-03-04T09:00:00Z",
    ));
    t.wait_until(|s| s.contains("Delta report")).unwrap();
    two.first.deliver(FakeMessage::new(
        "Erin",
        "Echo minutes",
        "2024-03-05T09:00:00Z",
    ));
    t.wait_until(|s| s.contains("Echo minutes") && s.contains("Delta report"))
        .unwrap();
    quit(t);
}

#[test]
fn compose_in_all_inboxes_goes_out_from_the_account_of_the_selected_thread() {
    let two = two();
    let mut t = postal(&two);
    t.wait_until(|s| s.contains("Alpha news") && s.contains("Bravo notes"))
        .unwrap();
    // Bravo notes, the second Account's, is the second row.
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("Body of Bravo notes")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains(" Draft ") && s.contains(OTHER))
        .unwrap();
    t.send(Key::Char('y')).unwrap();
    wait_for(|| two.smtp.1.received().len() == 1);
    assert!(two.smtp.0.received().is_empty());
    quit(t);
}

/// Message `n` of one Account, `n` hours and `minutes` after the epoch.
fn hourly(account: &str, n: i64, minutes: i64) -> FakeMessage {
    let date = chrono::DateTime::UNIX_EPOCH
        + chrono::Duration::hours(n)
        + chrono::Duration::minutes(minutes);
    let subject = format!("{account} {n}");
    FakeMessage::new("Sender", &subject, &date.to_rfc3339()).body(FakeBody::plain(&subject))
}

/// The Preview's title: the selection's place in the Message List.
fn place(t: &mut Terminal, place: &str) {
    t.wait_until(|s| s.row_text(0).contains(&format!(" {place} ")))
        .unwrap();
}

// A row below an Account's cache edge would hide that Account's own
// older mail, not yet fetched; each Backfill goes to the Account whose
// edge stops the list, until no Account has older mail.
#[test]
fn all_inboxes_ends_at_the_latest_cache_edge_until_a_backfill_lowers_it() {
    let first = (1..=600).map(|n| hourly("First", n, 0)).collect();
    // One below the first Account's first window of 500, one above it.
    let second = vec![hourly("Second", 50, 30), hourly("Second", 700, 0)];
    let two = two_with(first, second);
    let mut t = postal(&two);
    // The first's 500 newest, from hour 101, and the second's from hour 700.
    place(&mut t, "1/501");
    wait_for(|| two.first.commands().iter().any(|c| c == "IDLE"));
    wait_for(|| two.second.commands().iter().any(|c| c == "IDLE"));
    t.send(Key::Char('G')).unwrap();
    // The first's backfill reaches its oldest, the second's finds none older.
    t.wait_until(|s| s.row_text(0).contains("/602 ")).unwrap();
    t.send(Key::Char('G')).unwrap();
    place(&mut t, "602/602");
    t.wait_until(|s| s.contains("First 1\u{20}")).unwrap();
    quit(t);
}

#[test]
fn a_folder_of_the_second_account_names_it_in_the_preview_and_marks_read_there() {
    let two = two();
    let mut t = postal(&two);
    second_inbox(&mut t);
    t.wait_until(|s| s.contains("Body of Bravo notes") && s.contains(&format!("Account {OTHER}")))
        .unwrap();

    // Reading names it on each unfolded Message.
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| !s.contains("enter read") && s.contains(&format!("Account {OTHER}")))
        .unwrap();
    wait_for(|| two.second.seen("INBOX", two.bravo));
    assert!(
        !two.first.commands().iter().any(|c| c.contains("STORE")),
        "{:?}",
        two.first.commands()
    );
    quit(t);
}

#[test]
fn with_one_account_no_header_names_it() {
    let server = FakeImap::start(vec![
        FakeMessage::new("Alice", "Alpha news", "2024-03-02T09:00:00Z")
            .body(FakeBody::plain("Body of Alpha news")),
    ]);
    let home = Home::for_server(&server);
    let mut t = Terminal::builder()
        .size(120, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("Body of Alpha news"))
        .unwrap();
    assert!(!screen.contains("Account"), "{screen:?}");
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| !s.contains("enter read") && s.contains("Body of Alpha news"))
        .unwrap();
    assert!(!screen.contains("Account"), "{screen:?}");
    quit(t);
}

#[test]
fn a_delete_goes_to_the_trash_of_the_messages_account() {
    let two = two();
    let mut t = postal(&two);
    second_inbox(&mut t);
    t.send(Key::Backspace).unwrap();
    wait_for(|| two.second.subjects("Deleted Messages") == ["Bravo notes"]);
    assert_eq!(two.first.subjects("INBOX"), ["Alpha news"]);

    t.send(Key::Char('u')).unwrap();
    wait_for(|| two.second.subjects("INBOX") == ["Bravo notes"]);
    quit(t);
}

#[test]
fn a_search_matches_the_messages_of_every_account() {
    let two = two();
    let mut t = postal(&two);
    t.wait_until(|s| s.contains("Alpha news")).unwrap();
    // The second Account's rows are in the Cache once its Inbox is.
    t.send(Key::Char('f')).unwrap();
    t.wait_until(both_listed).unwrap();
    t.send(Key::Esc).unwrap();
    // A key read with `Esc` in one go is taken for Alt and the Esc is lost.
    t.wait_until(|s| picker(s).is_empty()).unwrap();
    t.send(Key::Char('/')).unwrap();
    t.send_str("bravo").unwrap();
    t.wait_until(|s| s.contains("Bravo notes") && !s.contains("Alpha news"))
        .unwrap();
    quit(t);
}

#[test]
fn compose_and_reply_go_out_from_the_account_of_the_shown_folder() {
    let two = two();
    let mut t = postal(&two);
    second_inbox(&mut t);
    for key in ['c', 'r'] {
        t.send(Key::Char(key)).unwrap();
        t.wait_until(|s| s.contains(" Draft ") && s.contains(OTHER))
            .unwrap();
        t.send(Key::Char('y')).unwrap();
        t.wait_until(|s| s.contains("Sent") && !s.contains(" Draft "))
            .unwrap();
    }

    let received = two.smtp.1.received();
    assert_eq!(received.len(), 2, "{received:?}");
    assert!(
        received.iter().all(|r| r.mail_from == OTHER),
        "{received:?}"
    );
    assert!(two.smtp.0.received().is_empty());
    // The answered Message is flagged on the server it reached.
    wait_for(|| two.second.answered("INBOX", two.bravo));
    quit(t);
}

#[test]
fn a_draft_of_an_account_gone_from_the_config_sends_from_the_first() {
    let two = two();
    let mut t = postal(&two);
    second_inbox(&mut t);
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains(" Draft ")).unwrap();
    t.send(Key::Char('d')).unwrap();
    t.wait_until(|s| s.contains("Saved to Drafts")).unwrap();
    quit(t);

    let config = fs::read_to_string(two.home.config_path()).unwrap();
    let (first, _) = config.split_once("[[account]]").unwrap();
    fs::write(two.home.config_path(), first).unwrap();
    let mut t = postal(&two);
    t.wait_until(|s| s.contains("Alpha news")).unwrap();
    t.send(Key::Char('f')).unwrap();
    t.send_str("local").unwrap();
    t.wait_until(|s| picked(s).first().is_some_and(|f| f == "Local Drafts"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(" Local Drafts ") && s.contains("Plans"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(" Draft ") && s.contains(EMAIL))
        .unwrap();
    t.send(Key::Char('y')).unwrap();
    // Queued for an Account no longer run, it would wait in the Outbox.
    wait_for(|| two.smtp.0.received().len() == 1);
    quit(t);
}

#[test]
fn local_drafts_names_the_sender_of_each_draft() {
    let two = two();
    let mut t = postal(&two);
    second_inbox(&mut t);
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains(" Draft ")).unwrap();
    t.send(Key::Char('d')).unwrap();
    t.send(Key::Char('f')).unwrap();
    t.wait_until(both_listed).unwrap();
    t.send_str("local").unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(" Local Drafts ") && s.contains(OTHER) && s.contains("Plans"))
        .unwrap();
    quit(t);
}

#[test]
fn the_status_names_the_account_in_the_worst_state() {
    let two = two();
    two.second.fail(Outage::Offline);
    let mut t = postal(&two);
    t.wait_until(|s| s.contains("Alpha news") && s.contains(&format!("{OTHER} · offline")))
        .unwrap();
    quit(t);
}

#[test]
fn esc_on_a_lost_secret_runs_without_that_account() {
    let two = two();
    fs::remove_file(two.home.secret_path_of(OTHER)).unwrap();
    let mut t = postal(&two);
    t.wait_until(|s| s.contains(&format!("Sign in to iCloud as {OTHER}")))
        .unwrap();
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| s.contains("Alpha news")).unwrap();
    t.send(Key::Char('f')).unwrap();
    let screen = t
        .snapshot_after(|s| picked(s).iter().any(|r| r == "Local Drafts"))
        .unwrap();
    assert!(!picked(&screen).iter().any(|r| r == OTHER), "{screen:?}");
    assert!(two.second.commands().is_empty());
    quit(t);
}
