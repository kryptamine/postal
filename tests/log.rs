#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fs, process::Command};

use support::{
    fake_imap::{ACCESS_TOKEN, FakeImap, FakeMessage},
    fake_oauth::REFRESH_TOKEN,
    home::Home,
    wait_for,
};
use termlens::{Key, Terminal};

fn messages() -> Vec<FakeMessage> {
    (1..=30)
        .map(|day| {
            FakeMessage::new(
                "Alice",
                &format!("Subject {day}"),
                &format!("2024-03-{day:02}T09:00:00Z"),
            )
        })
        .collect()
}

fn spawn(home: &Home, args: &[&str], env: &[(&str, &str)]) -> Terminal {
    let mut builder = home.terminal().size(80, 12).args(args);
    for (key, value) in env {
        builder = builder.env(key, value);
    }

    builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap()
}

/// Loads the Inbox, waits for the sync to idle, presses `keys`, quits,
/// and returns the run's log.
fn run(
    server: &FakeImap,
    home: &Home,
    args: &[&str],
    env: &[(&str, &str)],
    keys: &[Key],
) -> String {
    let idles = || server.commands().iter().filter(|c| *c == "IDLE").count();
    let before = idles();
    let mut t = spawn(home, args, env);
    t.wait_until(|s| s.contains(" · 1/")).unwrap();
    wait_for(|| idles() > before);
    for &key in keys {
        t.send(key).unwrap();
    }

    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    fs::read_to_string(home.log_path()).unwrap()
}

#[test]
fn each_run_keeps_the_previous_log_and_no_older_one() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let first = run(&server, &home, &[], &[], &[]);
    assert!(first.contains("starting"), "{first}");

    let second = run(&server, &home, &[], &[], &[]);
    let previous = home.log_path().with_extension("log.1");
    assert_eq!(fs::read_to_string(&previous).unwrap(), first);

    run(&server, &home, &[], &[], &[]);
    assert_eq!(fs::read_to_string(&previous).unwrap(), second);
    assert!(!home.log_path().with_extension("log.2").exists());
}

#[test]
fn a_full_log_ends_with_one_truncation_line() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let log = run(&server, &home, &["-vv"], &[("POSTAL_LOG_CAP", "4000")], &[]);
    assert!(log.ends_with("\nlog truncated\n"), "{log}");
    assert_eq!(log.matches("log truncated").count(), 1);
    assert!(log.len() <= 4000 + "log truncated\n".len(), "{}", log.len());
}

// The wire trace must stay useful (commands, elided literals) without ever
// carrying a token or a header body.
#[test]
fn the_wire_trace_never_carries_tokens_or_literals() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let log = run(&server, &home, &["-vv"], &[], &[]);
    assert!(log.contains("C: "), "{log}");
    assert!(log.contains("EXAMINE"), "{log}");
    assert!(log.contains(" bytes>"), "{log}");
    assert!(!log.contains(ACCESS_TOKEN), "{log}");
    assert!(!log.contains(REFRESH_TOKEN), "{log}");
    assert!(!log.contains("From: Alice"), "{log}");
}

// ADR 0001: an enabled event allocates, so scrolling logs nothing at the default level.
#[test]
#[cfg_attr(feature = "alloc-count", ignore = "alloc-count logs every frame")]
fn scrolling_logs_nothing_at_the_default_level() {
    let server = FakeImap::start(messages());
    let still = run(&server, &Home::for_server(&server), &[], &[], &[]);
    let keys = [Key::Char('j'); 20]
        .into_iter()
        .chain([Key::Char('G'), Key::Char('k'), Key::Char('g')])
        .collect::<Vec<_>>();
    let scrolled = run(&server, &Home::for_server(&server), &[], &[], &keys);
    assert_eq!(
        still.lines().count(),
        scrolled.lines().count(),
        "{scrolled}"
    );
}

#[test]
fn postal_log_overrides_the_flags() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let log = run(&server, &home, &["-vv"], &[("POSTAL_LOG", "warn")], &[]);
    assert!(log.is_empty(), "{log}");
}

#[test]
fn rust_log_is_ignored() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let log = run(&server, &home, &[], &[("RUST_LOG", "trace")], &[]);
    assert!(!log.contains("C: "), "{log}");
    assert!(log.contains("starting"), "{log}");
}

#[test]
fn help_names_the_log_and_what_the_trace_holds() {
    let home = Home::new("");
    let output = Command::new(env!("CARGO_BIN_EXE_postal"))
        .arg("--help")
        .env_clear()
        .envs(support::home::coverage())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(
        help.contains(&home.log_path().display().to_string()),
        "{help}"
    );
    assert!(
        help.contains("trace contains headers, not bodies or credentials"),
        "{help}"
    );
    assert!(!home.log_path().exists(), "--help opened the log");
}

#[test]
fn an_error_before_the_screen_is_logged_and_names_the_log() {
    let home = Home::new("email = 1\n");
    let output = Command::new(env!("CARGO_BIN_EXE_postal"))
        .env_clear()
        .envs(support::home::coverage())
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let log = fs::read_to_string(home.log_path()).unwrap();
    assert!(
        stderr.contains(&format!("Log: {}", home.log_path().display())),
        "{stderr}"
    );
    assert!(log.contains("ERROR"), "{log}");
    assert!(
        log.contains(&home.config_path().display().to_string()),
        "{log}"
    );
}

#[test]
fn an_unknown_argument_is_refused() {
    let home = Home::new("");
    let output = Command::new(env!("CARGO_BIN_EXE_postal"))
        .arg("--frobnicate")
        .env_clear()
        .envs(support::home::coverage())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--frobnicate"), "{stderr}");
}
