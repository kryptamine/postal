//! `postal login --app-password` asks for the email and an app password,
//! verifies them against the server, and only then writes config and secret.

#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

use support::{
    browser::Browser,
    fake_imap::{EMAIL, FakeImap, Outage, PASSWORD},
    home::Home,
};
use termlens::{Key, Terminal};

const APP_PASSWORDS: &str = "https://myaccount.google.com/apppasswords";

/// The app password as Google's page shows it, in groups of four.
const PASTED: &str = "abcd efgh ijkl mnop";

/// A config with no Account yet: the server and the file store, as only a
/// test sets them before the first login.
fn seeded(server: &FakeImap) -> Home {
    let home = Home::new(&format!(
        "[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n",
        server.port
    ));
    fs::remove_file(home.secret_path()).unwrap();
    home
}

fn spawn(home: &Home, browser: &Browser, args: &[&str]) -> Terminal {
    spawn_with(home, browser, args, &[])
}

fn spawn_with(home: &Home, browser: &Browser, args: &[&str], envs: &[(&str, &str)]) -> Terminal {
    Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("PATH", browser.path())
        .env("TZ", "UTC")
        .envs(envs.iter().copied())
        .args(args)
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn login(home: &Home, browser: &Browser) -> Terminal {
    spawn(home, browser, &["login", "--app-password"])
}

fn give_email(t: &mut Terminal) {
    t.wait_until(|s| s.contains("Email:")).unwrap();
    t.send_str(EMAIL).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("App password:")).unwrap();
}

fn give_password(t: &mut Terminal, password: &str) {
    t.paste(password).unwrap();
    t.send(Key::Enter).unwrap();
}

/// The secret file without its line break.
fn secret(home: &Home) -> Option<String> {
    let text = fs::read_to_string(home.secret_path()).ok()?;
    Some(text.trim_end().to_owned())
}

#[test]
fn a_verified_app_password_writes_config_and_secret() {
    let server = FakeImap::start(Vec::new());
    let home = seeded(&server);
    let browser = Browser::new();
    let mut t = login(&home, &browser);
    give_email(&mut t);
    let screen = t.screen();
    assert!(screen.contains("2-Step Verification"), "{screen:?}");
    assert!(screen.contains(APP_PASSWORDS), "{screen:?}");
    browser.wait_for(&[APP_PASSWORDS]);

    give_password(&mut t, PASTED);
    let screen = t.snapshot_after(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert!(!screen.contains("abcd"), "echoed the password: {screen:?}");

    assert_eq!(secret(&home).as_deref(), Some(PASSWORD));
    let mode = fs::metadata(home.secret_path())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    // Opening the Inbox is part of the check, not only the login.
    let commands = server.commands();
    let login = commands.iter().position(|c| c.starts_with("LOGIN"));
    let examine = commands.iter().position(|c| c.starts_with("EXAMINE"));
    assert!(login.is_some() && login < examine, "{commands:?}");

    // The written config must be one Postal starts from.
    let mut t = spawn(&home, &browser, &[]);
    t.wait_until(|s| s.contains(" unread ")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn a_rejected_password_prompts_again_and_writes_nothing() {
    let server = FakeImap::start(Vec::new());
    let home = seeded(&server);
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = login(&home, &browser);
    give_email(&mut t);

    give_password(&mut t, "wxyz wxyz wxyz wxyz");
    t.wait_until(|s| s.contains("refused")).unwrap();
    t.wait_until(|s| s.logical_text().matches("App password:").count() == 2)
        .unwrap();
    assert_eq!(secret(&home), None);
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), seed);

    give_password(&mut t, PASTED);
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some(PASSWORD));
}

#[test]
fn a_dropped_connection_exits_non_zero_and_writes_nothing() {
    let server = FakeImap::start(Vec::new());
    let home = seeded(&server);
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = login(&home, &browser);
    give_email(&mut t);

    server.fail(Outage::Offline);
    give_password(&mut t, PASTED);
    let screen = t.snapshot_after(|s| s.contains("postal:")).unwrap();
    assert!(!t.wait_exit().unwrap().success(), "{screen:?}");
    let prompts = screen.logical_text().matches("App password:").count();
    assert_eq!(prompts, 1, "prompted again: {screen:?}");
    assert_eq!(secret(&home), None);
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), seed);
}

#[test]
fn a_login_with_the_same_email_replaces_only_the_secret() {
    let server = FakeImap::start(Vec::new());
    let home = Home::for_server(&server);
    // Comments survive only if the config is left alone.
    let config = format!(
        "# hand-written\n{}",
        fs::read_to_string(home.config_path()).unwrap()
    );
    fs::write(home.config_path(), &config).unwrap();
    fs::write(home.secret_path(), "revoked\n").unwrap();
    let browser = Browser::new();
    let mut t = login(&home, &browser);
    give_email(&mut t);

    give_password(&mut t, PASTED);
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some(PASSWORD));
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
}

#[test]
fn ctrl_c_at_the_password_prompt_exits_and_writes_nothing() {
    let server = FakeImap::start(Vec::new());
    let home = seeded(&server);
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = login(&home, &browser);
    give_email(&mut t);

    t.send(Key::Ctrl('c')).unwrap();
    t.wait_until(|s| s.contains("login cancelled")).unwrap();
    assert!(!t.wait_exit().unwrap().success());
    assert_eq!(secret(&home), None);
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), seed);
}

// Replacing the Account also needs the Cache cleared, which login does not do.
#[test]
fn a_login_as_another_account_is_refused() {
    let server = FakeImap::start(Vec::new());
    let home = Home::for_server(&server);
    let config = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = login(&home, &browser);
    t.wait_until(|s| s.contains("Email:")).unwrap();
    t.send_str("other@example.com").unwrap();
    t.send(Key::Enter).unwrap();

    t.wait_until(|s| s.contains("already signed in as"))
        .unwrap();
    assert!(!t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some(PASSWORD));
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
}

// The keychain is never swapped for the file without the user choosing it.
#[test]
fn a_locked_keychain_hints_at_the_file_store_and_writes_no_config() {
    let server = FakeImap::start(Vec::new());
    let seed = format!(
        "[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n",
        server.port
    );
    let home = Home::new(&seed);
    fs::remove_file(home.secret_path()).unwrap();
    let browser = Browser::new();
    let locked = [("POSTAL_KEYCHAIN_FAIL", "the test locked the keychain")];
    let mut t = spawn_with(&home, &browser, &["login", "--app-password"], &locked);
    give_email(&mut t);

    give_password(&mut t, PASTED);
    t.wait_until(|s| s.contains("store = \"file\"")).unwrap();
    assert!(!t.wait_exit().unwrap().success());
    assert_eq!(secret(&home), None);
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), seed);
}

#[test]
fn login_without_a_method_names_the_app_password_flag() {
    let home = Home::new("");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_postal"))
        .arg("login")
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("postal login --app-password"), "{stderr}");
}
