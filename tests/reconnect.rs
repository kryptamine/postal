//! Losing the server and getting it back: the backoff, its countdown in the
//! status slot, and the resync after every reconnect.

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

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage, Outage, Stall},
    home::Home,
};
use termlens::{Color, Key, Screen, Terminal};

const YELLOW: Color = Color::Indexed(3);

fn postal(home: &Home) -> Terminal {
    Terminal::builder()
        .size(100, 20)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(15))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn idles(server: &FakeImap) -> usize {
    server.commands().iter().filter(|c| *c == "IDLE").count()
}

/// Starts Postal and waits until the sync is done and idling.
fn idling(server: &FakeImap, home: &Home) -> Terminal {
    let t = postal(home);
    wait_for(|| idles(server) > 0);
    t
}

fn hint_line(screen: &Screen) -> String {
    screen.row_text(screen.rows() - 1)
}

/// The status slot's text and whether it is drawn in yellow; `None` when
/// the hint line does not show `text`.
fn status(screen: &Screen, text: &str) -> Option<(String, bool)> {
    let line = hint_line(screen);
    let (before, from) = line.split_at(line.find(text)?);
    let column = before.chars().count();
    let yellow = screen
        .cell(screen.rows() - 1, u16::try_from(column).unwrap())
        .is_some_and(|cell| cell.style().fg == YELLOW);
    Some((from.trim_end().to_owned(), yellow))
}

/// The seconds a `retry in Ns` countdown shows.
fn countdown(screen: &Screen) -> Option<u64> {
    let line = hint_line(screen);
    let (_, rest) = line.split_once("retry in ")?;
    rest.split('s').next()?.parse().ok()
}

fn two_messages() -> Vec<FakeMessage> {
    vec![
        FakeMessage::new("Ada", "Older", "2024-11-17T09:00:00Z")
            .body(FakeBody::plain("Older body")),
        FakeMessage::new("Bob", "Newer", "2024-11-18T09:00:00Z")
            .body(FakeBody::plain("Newer body")),
    ]
}

fn log(home: &Home) -> String {
    fs::read_to_string(home.log_path()).unwrap()
}

#[test]
fn a_dropped_connection_counts_down_then_recovers_and_resyncs() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);
    t.wait_until(|s| s.contains("Newer body")).unwrap();

    server.fail(Outage::Offline);
    let screen = t
        .snapshot_after(|s| status(s, "offline · retry in").is_some())
        .unwrap();
    let (text, yellow) = status(&screen, "offline").unwrap();
    assert!(yellow, "{screen}");
    let first = countdown(&screen).unwrap();
    assert!(first <= 60, "{text}");

    // Refused again, the backoff grows past the first wait.
    t.wait_until(|s| countdown(s).is_some_and(|n| n >= 2))
        .unwrap();
    // The stale Message List stays usable.
    t.send(Key::Char('j')).unwrap();
    t.wait_until(|s| s.contains("Older body") && s.contains(" 2/2 "))
        .unwrap();

    server.deliver(FakeMessage::new(
        "Cyd",
        "Missed while away",
        "2024-11-19T09:00:00Z",
    ));
    server.clear_commands();
    server.recover();
    let screen = t
        .snapshot_after(|s| s.contains("Missed while away") && status(s, "retry").is_none())
        .unwrap();
    wait_for(|| idles(&server) > 0);
    quit(t);

    assert!(!hint_line(&screen).contains("offline"), "{screen}");
    let commands = server.commands();
    let examine = commands.iter().position(|c| c.starts_with("EXAMINE"));
    let idle = commands.iter().position(|c| c == "IDLE");
    assert!(examine.is_some() && examine < idle, "{commands:#?}");
    let log = log(&home);
    assert!(
        log.lines()
            .any(|l| l.contains("WARN") && l.contains("retrying in")),
        "{log}"
    );
}

// Gmail greets with `BYE [UNAVAILABLE]` while a backend is down: that is
// the server's trouble, not the network's.
#[test]
fn a_bye_shows_server_busy_and_is_logged() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);

    server.fail(Outage::Bye);
    let screen = t
        .snapshot_after(|s| status(s, "server busy · retry in").is_some())
        .unwrap();
    server.recover();
    t.wait_until(|s| status(s, "retry").is_none()).unwrap();
    quit(t);

    let (_, yellow) = status(&screen, "server busy").unwrap();
    assert!(yellow, "{screen}");
    let log = log(&home);
    assert!(
        log.lines()
            .any(|l| l.contains("WARN") && l.contains("BYE") && l.contains("UNAVAILABLE")),
        "{log}"
    );
}

#[test]
fn a_throttled_login_shows_server_busy() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);

    server.fail(Outage::Throttled);
    t.wait_until(|s| status(s, "server busy · retry in").is_some())
        .unwrap();
    server.recover();
    t.wait_until(|s| status(s, "retry").is_none()).unwrap();
    quit(t);

    let log = log(&home);
    assert!(
        log.lines()
            .any(|l| l.contains("WARN") && l.contains("THROTTLED")),
        "{log}"
    );
}

// A command the server refused, where nothing handles the refusal, ended
// Postal; a fresh connection starts over instead.
#[test]
fn a_refused_command_reconnects_instead_of_quitting() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);

    server.fail(Outage::Refusing);
    // The resync's fetch of it is the refused command.
    server.deliver(FakeMessage::new("Cyd", "Arrived", "2024-11-19T09:00:00Z"));
    t.wait_until(|s| status(s, "error · retry in").is_some())
        .unwrap();
    server.recover();
    t.wait_until(|s| s.contains("Arrived") && status(s, "retry").is_none())
        .unwrap();
    quit(t);

    let log = log(&home);
    assert!(
        log.lines()
            .any(|l| l.contains("WARN") && l.contains("Internal error")),
        "{log}"
    );
}

// Starting without a network must not end Postal: the Cache is shown and
// the sync keeps trying.
#[test]
fn starting_offline_shows_the_cache_and_keeps_trying() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    quit(idling(&server, &home));

    server.fail(Outage::Offline);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains("Newer") && status(s, "offline · retry in").is_some())
        .unwrap();
    server.recover();
    t.wait_until(|s| status(s, "retry").is_none()).unwrap();
    wait_for(|| idles(&server) > 1);
    quit(t);
}

#[test]
fn the_status_says_connecting_then_syncing_dimmed() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    server.stall(Stall::Greeting);
    let mut t = postal(&home);
    let connecting = t
        .snapshot_after(|s| status(s, "connecting…").is_some())
        .unwrap();

    server.stall(Stall::Fetch);
    let syncing = t
        .snapshot_after(|s| status(s, "syncing 0/2").is_some())
        .unwrap();
    server.release();
    t.wait_until(|s| s.contains("Newer") && status(s, "syncing").is_none())
        .unwrap();
    quit(t);

    for (screen, text) in [(&connecting, "connecting…"), (&syncing, "syncing")] {
        let line = hint_line(screen);
        let column = line.split_once(text).unwrap().0.chars().count();
        let cell = screen
            .cell(screen.rows() - 1, u16::try_from(column).unwrap())
            .unwrap();
        assert!(cell.style().dim, "{screen}");
        assert_eq!(cell.style().fg, Color::Default, "{screen}");
    }
}

// ^r cuts the backoff short for both connections, the sync's and the
// body connection's, each reconnecting before its countdown ends.
#[test]
fn ctrl_r_while_offline_reconnects_at_once() {
    let server = FakeImap::start(two_messages());
    let home = Home::for_server(&server);
    let mut t = idling(&server, &home);

    server.fail(Outage::Offline);
    t.wait_until(|s| countdown(s).is_some_and(|n| n >= 3))
        .unwrap();
    server.recover();
    let pressed = Instant::now();
    t.send(Key::Ctrl('r')).unwrap();
    let kicked = || {
        log(&home)
            .matches("a refresh was asked: reconnecting")
            .count()
    };
    wait_for(|| kicked() == 2);
    t.wait_until(|s| countdown(s).is_none()).unwrap();
    let waited = pressed.elapsed();
    quit(t);
    assert!(waited < Duration::from_secs(2), "{waited:?}");
}
