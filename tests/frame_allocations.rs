//! ADR 0001: a frame in steady state does not allocate. Built only with the
//! `alloc-count` feature, which makes the binary log allocations per frame.

#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::{
    fs,
    time::{Duration, Instant},
};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Terminal};

const MESSAGES: i64 = 60;

fn postal(home: &Home) -> Terminal {
    Terminal::builder()
        .size(80, 20)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

/// Read and Unread Messages, with subjects and Snippets long enough to be cut.
fn messages() -> Vec<FakeMessage> {
    (0..MESSAGES)
        .map(|i| {
            let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::days(19_000 + i);
            let subject =
                format!("Message {i} about a topic that needs far more room than a card has");
            let body = FakeBody::plain(&format!("The Snippet of {subject}, also cut"));
            let message = FakeMessage::new("Sender", &subject, &date.to_rfc3339()).body(body);
            if i % 3 == 0 {
                message.unread()
            } else {
                message
            }
        })
        .collect()
}

/// Allocations per frame logged so far.
fn frames(home: &Home) -> Vec<u64> {
    fs::read_to_string(home.log_path())
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains("frame drawn"))
        .map(|line| {
            let (_, count) = line.split_once("allocations=").unwrap();
            count.split_whitespace().next().unwrap().parse().unwrap()
        })
        .collect()
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The frames logged once no new one has come for a while.
fn settled_frames(home: &Home) -> Vec<u64> {
    let mut seen = frames(home);
    loop {
        std::thread::sleep(Duration::from_millis(300));
        let now = frames(home);
        if now.len() == seen.len() {
            return now;
        }

        seen = now;
    }
}

#[test]
fn scrolling_the_whole_list_from_the_cache_allocates_nothing_per_frame() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains(" unread ")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "LOGOUT"));
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    server.clear_commands();
    let mut t = postal(&home);
    wait_for(|| server.commands().iter().any(|c| c == "LOGOUT"));
    let loading = settled_frames(&home);
    // Reading the whole Cache allocates: proof the count is live.
    assert!(loading.iter().any(|&n| n > 0), "{loading:?}");

    // Fetching and rendering a Preview allocates; showing one already
    // rendered must not. The way down fetches every body, and the way back
    // renders those that came after the selection had moved on.
    let rows = usize::try_from(MESSAGES).unwrap();
    let down = vec![Key::Char('j'); rows];
    let up = vec![Key::Char('k'); rows];
    for (keys, title) in [(&down, format!(" {MESSAGES}/")), (&up, " 1/".to_owned())] {
        for &key in keys {
            t.send(key).unwrap();
        }

        t.wait_until(|s| s.contains(&title) && s.contains("also cut"))
            .unwrap();
        settled_frames(&home);
    }

    let warm = frames(&home);
    let back = [
        Key::Char('k'),
        Key::Ctrl('d'),
        Key::PageDown,
        Key::Ctrl('u'),
        Key::PageUp,
        Key::Char('G'),
        Key::Char('g'),
        Key::Char('?'),
        Key::Char('j'),
        Key::Char('?'),
        Key::Char('x'),
    ];
    for &key in &down {
        t.send(key).unwrap();
    }

    // Past a second idle: chrono rereads the time zone after one, allocating.
    std::thread::sleep(Duration::from_millis(1100));
    for &key in &back {
        t.send(key).unwrap();
    }

    let keys = down.len() + back.len();
    wait_for(|| frames(&home).len() >= warm.len() + keys);
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let all = frames(&home);
    let steady = &all[warm.len()..];
    assert_eq!(steady.len(), keys, "{all:?}");
    assert!(steady.iter().all(|&n| n == 0), "{steady:?}");
}
