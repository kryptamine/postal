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
    process::{Command, Output},
};

use support::home::Home;

fn postal(home: &Home) -> Output {
    Command::new(env!("CARGO_BIN_EXE_postal"))
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .output()
        .unwrap()
}

/// Runs Postal and returns its stderr, asserting it failed before drawing anything.
fn failure(home: &Home) -> String {
    let output = postal(home);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "drew a screen: {output:?}");
    String::from_utf8(output.stderr).unwrap()
}

const VALID_HEAD: &str = "email = \"me@example.com\"\n";

#[test]
fn a_syntax_error_names_the_path_and_line() {
    let home = Home::new(&format!("{VALID_HEAD}[imap\nhost = \"x\"\n"));
    let stderr = failure(&home);
    let at = format!("{}:2:", home.config_path().display());
    assert!(stderr.contains(&at), "{stderr}");
}

#[test]
fn a_wrong_value_names_the_path_and_line() {
    let home = Home::new(&format!(
        "{VALID_HEAD}\n[imap]\nport = \"993\"\n\n[secrets]\nstore = \"file\"\n"
    ));
    let stderr = failure(&home);
    let at = format!("{}:4:8:", home.config_path().display());
    assert!(stderr.contains(&at), "{stderr}");
}

#[test]
fn an_unknown_field_names_the_path_and_line() {
    let home = Home::new(&format!(
        "{VALID_HEAD}colour = true\n[secrets]\nstore = \"file\"\n"
    ));
    let stderr = failure(&home);
    let at = format!("{}:2:1:", home.config_path().display());
    assert!(stderr.contains(&at), "{stderr}");
    assert!(stderr.contains("colour"), "{stderr}");
}

#[test]
fn plaintext_is_refused_for_a_remote_host() {
    let home = Home::new(&format!(
        "{VALID_HEAD}[imap]\nhost = \"imap.gmail.com\"\ntls = false\n[secrets]\nstore = \"file\"\n"
    ));
    let stderr = failure(&home);
    let at = format!("{}:4:7:", home.config_path().display());
    assert!(stderr.contains(&at), "{stderr}");
    assert!(stderr.contains("loopback"), "{stderr}");
}

#[test]
fn plaintext_smtp_is_refused_for_a_remote_host() {
    let home = Home::new(&format!(
        "{VALID_HEAD}[smtp]\nhost = \"smtp.gmail.com\"\ntls = false\n[secrets]\nstore = \"file\"\n"
    ));
    let stderr = failure(&home);
    let at = format!("{}:4:7:", home.config_path().display());
    assert!(stderr.contains(&at), "{stderr}");
    assert!(stderr.contains("loopback"), "{stderr}");
}

#[test]
fn a_secret_other_users_can_read_is_refused() {
    let home = Home::new(&format!("{VALID_HEAD}[secrets]\nstore = \"file\"\n"));
    let secret = home.xdg_config_home().join("postal/secret");
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o644)).unwrap();
    let stderr = failure(&home);
    assert!(stderr.contains(&secret.display().to_string()), "{stderr}");
    assert!(stderr.contains("chmod 600"), "{stderr}");
}
