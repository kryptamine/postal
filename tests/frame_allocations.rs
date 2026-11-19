//! ADR 0001: a frame in steady state does not allocate. Built only with the
//! `alloc-count` feature, which makes the binary log allocations per frame.

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
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

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

/// Read and Unread Messages, with subjects and Snippets long enough to be
/// cut; every other one is HTML, drawn in the rendered View's colours.
fn messages() -> Vec<FakeMessage> {
    (0..MESSAGES)
        .map(|i| {
            let date = chrono::DateTime::UNIX_EPOCH + chrono::Duration::days(19_000 + i);
            let subject =
                format!("Message {i} about a topic that needs far more room than a card has");
            let snippet = format!("The Snippet of {subject}, also cut");
            let body = if i % 2 == 0 {
                FakeBody::plain(&snippet)
            } else {
                FakeBody::text(
                    "html",
                    "7bit",
                    format!(
                        "<table><tr><td style=\"background:#1a73e8;color:#fff;\
                         border:1px solid #ccc;padding:10px\">{snippet}</td></tr></table>"
                    ),
                )
            };

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

/// The selection's place in the Preview's title, as (row, rows).
fn position(screen: &Screen) -> (usize, usize) {
    (0..screen.rows())
        .flat_map(|row| {
            let text = screen.row_text(row);
            text.split(' ')
                .filter_map(|word| {
                    let (at, count) = word.split_once('/')?;
                    Some((at.parse().ok()?, count.parse().ok()?))
                })
                .collect::<Vec<_>>()
        })
        .next()
        .unwrap()
}

/// Moves one row and waits for that row's Preview: scrolling on without
/// waiting fetches only the bodies of the rows it stops on.
fn step(t: &mut Terminal, key: Key) {
    let (at, count) = position(&t.screen());
    let next = if key == Key::Char('j') {
        (at + 1).min(count)
    } else {
        at.saturating_sub(1).max(1)
    };

    t.send(key).unwrap();
    t.wait_until(|s| position(s) == (next, count) && !s.contains("Loading…"))
        .unwrap();
}

#[test]
fn scrolling_the_whole_list_from_the_cache_allocates_nothing_per_frame() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    t.wait_until(|s| s.contains(" unread ")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    server.clear_commands();
    let mut t = postal(&home);
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    let loading = settled_frames(&home);
    // Reading the whole Cache allocates: proof the count is live.
    assert!(loading.iter().any(|&n| n > 0), "{loading:?}");

    // Fetching and rendering a Preview allocates; showing one already
    // rendered must not. The way down fetches every body, and the way back
    // renders the neighbours each stop left behind.
    let rows = usize::try_from(MESSAGES).unwrap();
    let down = vec![Key::Char('j'); rows];
    let up = vec![Key::Char('k'); rows];
    for keys in [&down, &up] {
        for &key in keys {
            step(&mut t, key);
        }

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

// ADR 0001 exempts typing a Query, not moving through what it matched:
// the highlights are drawn from buffers kept between frames.
#[test]
fn scrolling_a_narrowed_list_allocates_nothing_per_frame() {
    let server = FakeImap::start(messages());
    let home = Home::for_server(&server);
    let mut t = postal(&home);
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    t.wait_until(|s| s.contains("also cut")).unwrap();
    t.send(Key::Char('/')).unwrap();
    t.send_str("message 1").unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.row_text(s.rows() - 1).contains("/message 1  "))
        .unwrap();

    let rows = usize::try_from(MESSAGES).unwrap();
    let down = vec![Key::Char('j'); rows];
    let up = vec![Key::Char('k'); rows];
    for keys in [&down, &up] {
        for &key in keys {
            step(&mut t, key);
        }

        settled_frames(&home);
    }

    let warm = frames(&home);
    for &key in down.iter().chain(&up) {
        t.send(key).unwrap();
    }

    let keys = down.len() + up.len();
    wait_for(|| frames(&home).len() >= warm.len() + keys);
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let all = frames(&home);
    let steady = &all[warm.len()..];
    assert_eq!(steady.len(), keys, "{all:?}");
    assert!(steady.iter().all(|&n| n == 0), "{steady:?}");
}
