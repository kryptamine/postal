#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    process::{Command, Output},
    time::Duration,
};

use support::{
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Terminal};

const LOCKED: &str = "the test locked the keychain";

/// Runs Postal with the keychain refusing access.
fn postal_locked_keychain(home: &Home) -> Output {
    Command::new(env!("CARGO_BIN_EXE_postal"))
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("POSTAL_KEYCHAIN_FAIL", LOCKED)
        .output()
        .unwrap()
}

/// Asserts Postal failed before drawing anything, with the hint to the file store.
fn assert_hints_at_the_file_store(home: &Home) {
    let output = postal_locked_keychain(home);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "drew a screen: {output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(LOCKED), "{stderr}");
    assert!(stderr.contains("store = \"file\""), "{stderr}");
    assert!(
        stderr.contains(&home.config_path().display().to_string()),
        "{stderr}"
    );
}

const HEAD: &str = "email = \"me@example.com\"\nauth = \"app-password\"\n";

// Home also writes a valid secret file: reading it instead would be the
// silent fallback to plaintext the user never chose.
#[test]
fn the_keychain_is_the_default_store_and_never_falls_back_to_the_file() {
    let home = Home::new(HEAD);
    assert_hints_at_the_file_store(&home);
}

#[test]
fn a_locked_keychain_hints_at_the_file_store() {
    let home = Home::new(&format!("{HEAD}[secrets]\nstore = \"keychain\"\n"));
    assert_hints_at_the_file_store(&home);
}

#[test]
fn an_empty_secrets_table_uses_the_keychain() {
    let home = Home::new(&format!("{HEAD}[secrets]\n"));
    assert_hints_at_the_file_store(&home);
}

// The fake server accepts only the password in the secret file, so the
// Inbox showing proves the file was read and the keychain left alone.
#[test]
fn the_file_store_never_touches_the_keychain() {
    let server = FakeImap::start(vec![FakeMessage::new(
        "Alice",
        "Hello",
        "2024-03-02T09:00:00Z",
    )]);
    let home = Home::for_server(&server);
    let mut t = Terminal::builder()
        .size(80, 25)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("POSTAL_KEYCHAIN_FAIL", LOCKED)
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();

    t.wait_until(|s| s.contains("Hello")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn an_unknown_store_names_the_path_and_line() {
    let home = Home::new(&format!("{HEAD}[secrets]\nstore = \"vault\"\n"));
    let stderr = String::from_utf8(postal_locked_keychain(&home).stderr).unwrap();
    let at = format!("{}:4:9:", home.config_path().display());
    assert!(stderr.contains(&at), "{stderr}");
}
