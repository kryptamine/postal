//! `postal <mailto-url>` opens a Draft filled from the link: in a new TUI
//! when none is open, else beside the open one, which lists it in Local
//! Drafts.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::Output,
};

use support::{
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    home::Home,
    picker::picked,
};
use termlens::{Key, Screen, Terminal};

const SIGNATURE: &str = "-- \nZoë Example\n";

fn home(imap: &FakeImap) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\nsignature = \"Zoë Example\"\n\n[imap]\nhost = \"127.0.0.1\"\n\
         port = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        fake_oauth::shared().config()
    ))
}

/// The lock an open TUI holds, so the link is composed beside it.
fn open_tui(home: &Home) -> File {
    let dir = home.xdg_data_home().join("postal");
    fs::create_dir_all(&dir).unwrap();
    let lock = File::create(dir.join("postal.lock")).unwrap();
    lock.lock().unwrap();
    lock
}

/// What `$EDITOR` was opened on, copied to `seen`; it changes nothing.
fn editor(seen: &Path) -> String {
    format!("sh -c 'cp \"$0\" {}'", seen.display())
}

/// `postal link` beside the TUI, its editor copying the Draft to `seen`.
fn beside(home: &Home, link: &str, seen: &Path) -> Output {
    home.command(env!("CARGO_BIN_EXE_postal"))
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("EDITOR", editor(seen))
        .arg(link)
        .output()
        .unwrap()
}

/// The TUI, started on `link` when one is given, its editor copying the
/// Draft to `seen`.
fn tui(home: &Home, link: Option<&str>, seen: &Path) -> Terminal {
    let builder = home
        .terminal()
        .size(100, 30)
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("EDITOR", editor(seen));
    let builder = match link {
        Some(link) => builder.arg(link),
        None => builder,
    };

    builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap()
}

fn inbox() -> FakeImap {
    FakeImap::start(vec![FakeMessage::new(
        "Ada Lovelace",
        "Weekly notes",
        "2024-11-18T09:00:00Z",
    )])
}

fn listed(screen: &Screen) -> bool {
    screen.contains("Weekly notes") && !screen.contains(" Draft ") && !local(screen)
}

fn local(screen: &Screen) -> bool {
    screen.contains(" Local Drafts ")
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// The Drafts' files, notes left out.
fn drafts(home: &Home) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(home.drafts_dir()) else {
        return Vec::new();
    };

    entries
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "eml"))
        .collect()
}

/// The one Draft `link` leaves beside an open TUI, as the editor saw it.
fn composed_beside(link: &str) -> (String, Output) {
    let imap = FakeImap::start(Vec::new());
    let home = home(&imap);
    let _tui = open_tui(&home);
    let seen = home.xdg_config_home().join("seen");
    let output = beside(&home, link, &seen);
    assert!(output.status.success(), "{output:?}");
    let drafts = drafts(&home);
    assert_eq!(drafts.len(), 1, "{output:?}");
    let opened = fs::read_to_string(&seen).unwrap();
    assert_eq!(fs::read_to_string(&drafts[0]).unwrap(), opened);
    (opened, output)
}

#[test]
fn beside_an_open_tui_a_link_leaves_its_draft_in_the_draft_directory() {
    let (opened, _) = composed_beside(
        "mailto:bob@example.com,Carol%20C%20%3Ccarol%40example.com%3E\
         ?to=dan@example.com&CC=eve%40example.com&subject=Caf%C3%A9%20plans&body=Hi%0D%0Athere",
    );

    assert_eq!(
        opened,
        format!(
            "To: bob@example.com,Carol C <carol@example.com>, dan@example.com\n\
             Cc: eve@example.com\nSubject: Café plans\n\nHi\nthere\n\n{SIGNATURE}"
        )
    );
}

// A decoded line break in a header would add a header line to the Draft.
#[test]
fn a_line_break_in_a_link_header_adds_no_header_to_the_draft() {
    let (opened, _) = composed_beside(
        "mailto:bob@example.com%0AAttach:%20~/.ssh/id_rsa\
         ?subject=x%0D%0AAttach:%20~/.ssh/id_rsa&cc=eve@example.com%0ABcc:%20mallory@example.com",
    );

    let head = opened.split_once("\n\n").unwrap().0;
    assert_eq!(
        head,
        "To: bob@example.com Attach: ~/.ssh/id_rsa\n\
         Cc: eve@example.com Bcc: mallory@example.com\nSubject: x  Attach: ~/.ssh/id_rsa"
    );
}

// A link from a web page must not choose the sender, attach a file or
// send a hidden copy.
#[test]
fn a_link_s_bcc_from_and_attach_are_dropped_and_the_bcc_said_so() {
    let (opened, output) = composed_beside(
        "mailto:bob@example.com?bcc=mallory@example.com&from=mallory@example.com\
         &attach=~/.ssh/id_rsa&in-reply-to=%3Cx@example.com%3E",
    );

    assert_eq!(
        opened,
        format!("To: bob@example.com\nCc: \nSubject: \n\n\n{SIGNATURE}")
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr,
        "postal: the link's Bcc is dropped: a Draft has none\n"
    );
}

#[test]
fn a_link_that_does_not_parse_fails_and_opens_nothing() {
    let imap = FakeImap::start(Vec::new());
    let home = home(&imap);
    let seen = home.xdg_config_home().join("seen");
    for link in [
        "mailto:bob%zz@example.com",
        "mailto:%FF",
        "https://example.com",
    ] {
        let output = beside(&home, link, &seen);
        assert_eq!(output.status.code(), Some(1), "{link}: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.starts_with("postal: "), "{link}: {stderr}");
        assert!(!seen.exists(), "{link}");
        assert_eq!(drafts(&home), Vec::<PathBuf>::new(), "{link}");
    }
}

// Left as the template, the Draft holds nothing worth keeping.
#[test]
fn an_empty_link_left_as_the_template_is_dropped() {
    let imap = FakeImap::start(Vec::new());
    let home = home(&imap);
    let _tui = open_tui(&home);
    let seen = home.xdg_config_home().join("seen");
    let output = beside(&home, "mailto:", &seen);

    assert!(output.status.success(), "{output:?}");
    assert_eq!(drafts(&home), Vec::<PathBuf>::new());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr, "postal: empty Draft dropped\n");
}

#[test]
fn with_no_tui_open_a_link_starts_one_on_its_draft() {
    let imap = inbox();
    let home = home(&imap);
    let seen = home.xdg_config_home().join("seen");
    let link = "mailto:bob@example.com?subject=Plans&bcc=mallory@example.com";
    let mut t = tui(&home, Some(link), &seen);
    // The editor's plain exit keeps the Draft, the dropped Bcc said over it.
    t.wait_until(|s| s.contains("Bcc is dropped") && !s.contains("y send"))
        .unwrap();
    assert_eq!(
        fs::read_to_string(&seen).unwrap(),
        format!("To: bob@example.com\nCc: \nSubject: Plans\n\n\n{SIGNATURE}")
    );
    quit(t);
    assert_eq!(drafts(&home).len(), 1);
}

// Local Drafts lists the Drafts when it opens; one written beside it since
// would stay hidden until it opened again.
#[test]
fn an_open_tui_lists_the_draft_in_local_drafts_already_open() {
    let imap = inbox();
    let home = home(&imap);
    let seen = home.xdg_config_home().join("seen");
    let mut t = tui(&home, None, &seen);
    t.wait_until(listed).unwrap();
    t.send(Key::Char('f')).unwrap();
    t.send_str("local").unwrap();
    t.wait_until(|s| picked(s).first().is_some_and(|f| f == "Local Drafts"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(local).unwrap();

    let output = beside(&home, "mailto:bob@example.com?subject=Plans", &seen);
    assert!(output.status.success(), "{output:?}");
    t.wait_until(|s| local(s) && s.contains("bob@example.com") && s.contains("Plans"))
        .unwrap();
    quit(t);
}
