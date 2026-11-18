//! `|` hands the Message to `$PAGER`, and `Ctrl-z` to the shell; the
//! screen comes back whole either way.

#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::{path::Path, time::Duration};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal, TerminalBuilder};

const HTML: &str = "<p>Read <a href=\"https://example.com/notes\">the notes</a> \
    and <a href=\"https://example.com/agenda\"><b>the agenda</b></a>.</p>\
    <p>See you <i>soon</i>.</p>";

/// `PATH` stays, for `sh` to find the pager's commands.
fn terminal(home: &Home) -> TerminalBuilder {
    Terminal::builder()
        .size(80, 24)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
}

fn postal(home: &Home, envs: &[(&str, &Path)], no_color: bool) -> Terminal {
    let mut builder = terminal(home);
    for (key, value) in envs {
        builder = builder.env(key, value);
    }

    if no_color {
        builder = builder.env("NO_COLOR", "1");
    }

    builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap()
}

fn server() -> FakeImap {
    server_with(HTML)
}

fn server_with(html: &str) -> FakeImap {
    let body = FakeBody::text("html", "7bit", html);
    let message = FakeMessage::new("Ada Lovelace", "Weekly notes", "2024-11-18T09:00:00Z");
    FakeImap::start(vec![message.body(body)])
}

fn main_screen(screen: &Screen) -> bool {
    screen.alternate_screen() && screen.contains(" 1/1 ") && screen.contains("the agenda")
}

/// The escape character made visible, so a snapshot shows the styling.
fn visible(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace('\x1b', "␛")
}

/// What a recording `$PAGER` read of `html` after `keys`, once the TUI
/// took the screen back.
fn paged(html: &str, keys: &[Key], no_color: bool) -> String {
    let server = server_with(html);
    let home = Home::for_server(&server);
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("tmp");
    std::fs::create_dir(&tmp).unwrap();
    let record = dir.path().join("paged");
    // Holds the pager open until the test answers on the terminal, so the
    // suspended screen can be seen.
    let pager = format!(
        "cat > '{}'; printf 'pager done'; read answer < /dev/tty",
        record.display()
    );
    let mut t = postal(
        &home,
        &[("PAGER", Path::new(&pager)), ("TMPDIR", &tmp)],
        no_color,
    );
    t.wait_until(main_screen).unwrap();

    for key in keys {
        t.send(*key).unwrap();
    }

    t.wait_until(|s| !s.alternate_screen() && s.contains("pager done"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(main_screen).unwrap();
    // The TUI still takes keys after the pager.
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let written: Vec<_> = std::fs::read_dir(&tmp).unwrap().collect();
    assert!(written.is_empty(), "{written:?}");
    visible(&std::fs::read(&record).unwrap())
}

// The pager copy keeps the Preview's styling and lists every URL, since
// the pager has no hint mode and URLs must stay copyable.
#[test]
fn pipe_hands_the_message_with_link_footnotes_to_the_pager() {
    insta::assert_snapshot!("colour", paged(HTML, &[Key::Char('|')], false));
}

#[test]
fn no_color_strips_the_colours_from_the_pager_copy() {
    insta::assert_snapshot!("no_color", paged(HTML, &[Key::Char('|')], true));
}

#[test]
fn the_full_screen_message_goes_to_the_pager_too() {
    let keys = [Key::Enter, Key::Char('|')];
    assert_eq!(
        paged(HTML, &keys, true),
        paged(HTML, &[Key::Char('|')], true)
    );
}

// Run straight under the terminal, Postal would lead an orphaned process
// group, where the kernel discards SIGTSTP. An interactive shell has job
// control; it goes on past a stopped Postal, and `fg` continues it.
const JOB_SHELL: &str = "\"$POSTAL\"; printf 'postal stopped'; read answer; fg";

#[test]
fn ctrl_z_stops_with_the_terminal_restored_and_redraws_after_fg() {
    let server = server();
    let home = Home::for_server(&server);
    let mut t = terminal(&home)
        .env("POSTAL", env!("CARGO_BIN_EXE_postal"))
        .args(["-i", "-c", JOB_SHELL])
        .spawn("sh")
        .unwrap();
    t.wait_until(main_screen).unwrap();

    t.send(Key::Ctrl('z')).unwrap();
    t.wait_until(|s| !s.alternate_screen() && s.contains("postal stopped"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(main_screen).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}
