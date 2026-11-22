//! Settings, opened with `,`: each change reaches `config.toml` at once,
//! Icons redraw the screen, and unlinking an Account leaves no trace of it.

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
    fake_imap::{APP_PASSWORD, EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    home::Home,
};
use termlens::{Key, Terminal};

const OTHER: &str = "me@icloud.example";
const UNREAD: &str = "\u{f01ee}";

fn inbox() -> FakeImap {
    FakeImap::start(vec![
        FakeMessage::new("Ada", "Weekly notes", "2024-11-18T09:00:00Z")
            .body(FakeBody::plain("Notes"))
            .unread(),
    ])
}

fn postal(home: &Home, keychain_fails: bool) -> Terminal {
    let builder = Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10));
    let builder = if keychain_fails {
        builder.env("POSTAL_KEYCHAIN_FAIL", "locked")
    } else {
        builder
    };

    builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap()
}

/// Opens Settings once the Inbox shows.
fn settings(t: &mut Terminal) {
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    t.send(Key::Char(',')).unwrap();
    t.wait_until(|s| s.contains("Settings") && s.contains("Secrets"))
        .unwrap();
}

fn config(home: &Home) -> String {
    fs::read_to_string(home.config_path()).unwrap()
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

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

// Icons change without leaving the screen, and stay changed next run.
#[test]
fn icons_cycle_and_redraw_at_once() {
    let server = inbox();
    let home = Home::for_server(&server);
    let mut t = postal(&home, false);
    settings(&mut t);
    // The Account, its Signature, Add account, then Icons.
    t.send(Key::Down).unwrap();
    t.send(Key::Down).unwrap();
    t.send(Key::Down).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("nerd")).unwrap();
    assert!(
        config(&home).contains("icons = \"nerd\""),
        "{}",
        config(&home)
    );

    t.send(Key::Esc).unwrap();
    t.wait_until(|s| s.contains(UNREAD) && !s.contains("Secrets"))
        .unwrap();
    quit(t);
}

// The Name goes to `config.toml` and the TUI opens again with it.
#[test]
fn the_name_is_typed_in_place() {
    let server = inbox();
    let home = Home::for_server(&server);
    let mut t = postal(&home, false);
    settings(&mut t);
    for _ in 0..4 {
        t.send(Key::Char('j')).unwrap();
    }

    t.send(Key::Enter).unwrap();
    // `q` and `j` are the Name's while it is typed.
    t.send_str("Ada jq").unwrap();
    t.send(Key::Enter).unwrap();
    wait_for(|| config(&home).contains("name = \"Ada jq\""));

    t.send(Key::Char('q')).unwrap();
    t.wait_until(|s| s.contains("Weekly notes") && !s.contains("Secrets"))
        .unwrap();
    quit(t);
}

// A change edits its own key: the user's comments stay where they were.
#[test]
fn a_change_keeps_the_comments() {
    let server = inbox();
    let home = Home::for_server(&server);
    let seed = format!("# hand-written\n{}", config(&home));
    fs::write(home.config_path(), &seed).unwrap();
    let mut t = postal(&home, false);
    settings(&mut t);
    t.send(Key::Down).unwrap();
    t.send(Key::Down).unwrap();
    t.send(Key::Down).unwrap();
    t.send(Key::Enter).unwrap();
    wait_for(|| config(&home).contains("icons = \"nerd\""));
    assert!(
        config(&home).starts_with("# hand-written\n"),
        "{}",
        config(&home)
    );
    // Back to the default, the key goes and the file is as it was.
    t.send(Key::Enter).unwrap();
    t.send(Key::Enter).unwrap();
    wait_for(|| config(&home) == seed);
    quit(t);
}

// A store that refuses the secrets leaves every secret where it was.
#[test]
fn a_refused_store_switch_changes_nothing() {
    let server = inbox();
    let home = Home::for_server(&server);
    let before = config(&home);
    let mut t = postal(&home, true);
    settings(&mut t);
    for _ in 0..5 {
        t.send(Key::Char('j')).unwrap();
    }

    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("keychain unavailable"))
        .unwrap();
    assert_eq!(config(&home), before);
    assert!(home.secret_path().exists());
    quit(t);
}

// Add account leaves for the Provider list, and back again on Esc.
#[test]
fn add_account_opens_the_onboarding() {
    let server = inbox();
    let home = Home::for_server(&server);
    let mut t = postal(&home, false);
    settings(&mut t);
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("iCloud") && !s.contains("Weekly notes"))
        .unwrap();

    t.send(Key::Esc).unwrap();
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    quit(t);
}

/// The first Account at a Gmail-like server, the second at an iCloud-like one.
fn two(first: &FakeImap, second: &FakeImap) -> Home {
    second.as_icloud();
    let server = |port: u16| format!("host = \"127.0.0.1\"\nport = {port}\ntls = false\n");
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\n{}\n[secrets]\nstore = \"file\"\n{}\n\
         [[account]]\nemail = \"{OTHER}\"\nprovider = \"icloud\"\n\n[account.imap]\n{}",
        server(first.port),
        fake_oauth::shared().config(),
        server(second.port),
    ));
    let secret = home.secret_path_of(OTHER);
    fs::write(&secret, format!("{APP_PASSWORD}\n")).unwrap();
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
    home
}

// An unlinked Account leaves the config, its secret and the Cache, and the
// TUI opens again with the others.
#[test]
fn unlinking_an_account_forgets_it() {
    let first = inbox();
    let second = FakeImap::start(vec![FakeMessage::new(
        "Bob",
        "Bravo notes",
        "2024-11-17T09:00:00Z",
    )]);
    let home = two(&first, &second);
    let mut t = postal(&home, false);
    t.wait_until(|s| s.contains("Bravo notes")).unwrap();
    settings(&mut t);
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Char('d')).unwrap();
    t.wait_until(|s| s.contains(&format!("Unlink {OTHER}")))
        .unwrap();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Weekly notes") && !s.contains("Bravo notes"))
        .unwrap();

    assert!(!config(&home).contains(OTHER), "{}", config(&home));
    assert!(config(&home).contains(EMAIL), "{}", config(&home));
    assert!(!home.secret_path_of(OTHER).exists());
    assert!(home.secret_path().exists());
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let left: i64 = cache
        .query_row(
            "SELECT count(*) FROM message WHERE account = ?1",
            [OTHER],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(left, 0);
    quit(t);
}

// Any key but `y` keeps the Account.
#[test]
fn n_keeps_the_account() {
    let first = inbox();
    let second = FakeImap::start(vec![]);
    let home = two(&first, &second);
    let before = config(&home);
    let mut t = postal(&home, false);
    settings(&mut t);
    t.send(Key::Char('d')).unwrap();
    t.wait_until(|s| s.contains(&format!("Unlink {EMAIL}")))
        .unwrap();
    t.send(Key::Char('n')).unwrap();
    t.wait_until(|s| !s.contains("Unlink")).unwrap();
    assert_eq!(config(&home), before);
    quit(t);
}
