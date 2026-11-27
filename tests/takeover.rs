//! A `postal` started while a TUI is open in another terminal asks to move
//! it here: the old TUI quits saying where it went, finishing a Send under
//! way first, and a folder sync holding the lock is waited for instead.

use crate::support;

use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

use support::{
    editor::sending,
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
    picker::picked,
    quit, wait_for,
};
use tempfile::TempDir;
use termlens::{Key, Screen, Signal, Terminal};

const WRITTEN: &str = "To: Bob Example <bob@example.com>\nSubject: Plans\n\nHi Bob\n";

const ASKED: &str = "Move it here?";

fn home(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[smtp]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        imap.port,
        smtp.port,
        fake_oauth::shared().config()
    ))
}

fn inbox() -> FakeImap {
    FakeImap::start(vec![FakeMessage::new(
        "Ada Lovelace",
        "Weekly notes",
        "2024-11-18T09:00:00Z",
    )])
}

/// `$EDITOR`, copying `WRITTEN` over the Draft once `go` exists.
struct Editor {
    dir: TempDir,
}

impl Editor {
    fn new() -> Self {
        let editor = Self {
            dir: tempfile::tempdir().unwrap(),
        };

        fs::write(editor.dir.path().join("next"), WRITTEN).unwrap();
        editor
    }

    /// Writes at once.
    fn go(&self) {
        fs::write(self.dir.path().join("go"), "").unwrap();
    }

    fn command(&self) -> String {
        let (go, next) = (self.dir.path().join("go"), self.dir.path().join("next"));
        format!(
            "sh -c 'while [ ! -e {} ]; do sleep 0.05; done; cp {} \"$0\"'",
            go.display(),
            next.display()
        )
    }
}

fn postal(home: &Home, editor: &str) -> Terminal {
    (home.terminal())
        .size(100, 30)
        .env("EDITOR", editor)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn listed(screen: &Screen) -> bool {
    screen.contains("Weekly notes") && !screen.contains(ASKED)
}

/// The second TUI's answer `y`, once it asks.
fn moved_here(second: &mut Terminal) {
    second.wait_until(|s| s.contains(ASKED)).unwrap();
    second.send(Key::Char('y')).unwrap();
}

/// What the first TUI left in its terminal as it quit.
fn left(first: &mut Terminal) -> String {
    assert!(first.wait_exit().unwrap().success());
    first.screen().text()
}

fn drafts(home: &Home) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(home.drafts_dir()) else {
        return Vec::new();
    };

    entries
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "eml"))
        .collect()
}

#[test]
fn a_second_postal_takes_the_session_over_and_the_first_says_where_it_went() {
    let (imap, smtp, editor) = (inbox(), FakeSmtp::start(), Editor::new());
    let home = home(&imap, &smtp);
    let mut first = postal(&home, &editor.command());
    first.wait_until(listed).unwrap();

    let mut second = postal(&home, &editor.command());
    let asked = second.wait_until_for(|s| s.contains(ASKED), Duration::from_secs(10));
    asked.unwrap();
    assert!(second.screen().contains("Postal is open in /dev/"));
    second.send(Key::Char('y')).unwrap();

    let text = left(&mut first);
    assert!(text.contains("postal moved to /dev/"), "{text}");
    second.wait_until(listed).unwrap();
    quit(second);
}

// Declining must leave the open session as it was.
#[test]
fn declining_the_move_leaves_the_first_running() {
    let (imap, smtp, editor) = (inbox(), FakeSmtp::start(), Editor::new());
    let home = home(&imap, &smtp);
    let mut first = postal(&home, &editor.command());
    first.wait_until(listed).unwrap();

    let mut second = postal(&home, &editor.command());
    second.wait_until(|s| s.contains(ASKED)).unwrap();
    second.send(Key::Char('n')).unwrap();
    assert!(second.wait_exit().unwrap().success());

    first.send(Key::Char('j')).unwrap();
    first.wait_until(listed).unwrap();
    quit(first);
}

// A Send cut short by the move would leave a Message SMTP took to go out
// again from the second TUI.
#[test]
fn a_send_under_way_finishes_before_the_move_and_goes_out_once() {
    let (imap, smtp, editor) = (inbox(), FakeSmtp::start(), Editor::new());
    smtp.stall(Duration::from_secs(2));
    editor.go();
    let home = home(&imap, &smtp);
    let mut first = postal(&home, &sending(&editor.command()));
    first.wait_until(listed).unwrap();
    first.send(Key::Char('c')).unwrap();
    wait_for(|| smtp.received().len() == 1);
    let idles = || imap.commands().iter().filter(|c| *c == "IDLE").count();
    let before = idles();

    let mut second = postal(&home, &editor.command());
    moved_here(&mut second);
    let text = left(&mut first);
    assert!(text.contains("postal moved to"), "{text}");
    second.wait_until(listed).unwrap();
    // A sync sends what the Outbox holds before its first IDLE.
    wait_for(|| idles() > before);
    assert_eq!(smtp.received().len(), 1);
    quit(second);
}

// The editor runs on in the first terminal; what it writes must not be lost.
#[test]
fn with_the_editor_open_the_session_moves_at_once_and_the_draft_follows() {
    let (imap, smtp, editor) = (inbox(), FakeSmtp::start(), Editor::new());
    let home = home(&imap, &smtp);
    let mut first = postal(&home, &editor.command());
    first.wait_until(listed).unwrap();
    first.send(Key::Char('c')).unwrap();
    wait_for(|| drafts(&home).len() == 1);

    let mut second = postal(&home, &editor.command());
    second.wait_until(|s| s.contains(ASKED)).unwrap();
    assert!(
        second
            .screen()
            .contains("the editor or the pager open there")
    );
    second.send(Key::Char('y')).unwrap();
    second.wait_until(listed).unwrap();

    editor.go();
    let text = left(&mut first);
    assert!(text.contains("postal moved to /dev/"), "{text}");
    let draft = drafts(&home);
    assert_eq!(draft.len(), 1);
    assert_eq!(fs::read_to_string(&draft[0]).unwrap(), WRITTEN);
    second.send(Key::Char('f')).unwrap();
    second.send_str("local").unwrap();
    second
        .wait_until(|s| picked(s).first().is_some_and(|f| f == "Local Drafts"))
        .unwrap();
    second.send(Key::Enter).unwrap();
    second.wait_until(|s| s.contains("Plans")).unwrap();
    quit(second);
}

/// A process holding the lock as `kind` names it, ignoring SIGTERM; it
/// says so on stdout once it holds it.
fn holder(home: &Home, kind: &str) -> Child {
    let dir = home.xdg_data_home().join("postal");
    fs::create_dir_all(&dir).unwrap();
    let script = format!(
        "use Fcntl ':flock'; use IO::Handle; $| = 1; \
         open(my $f, '+>>', $ARGV[0]) or die; flock($f, LOCK_EX) or die; \
         $SIG{{TERM}} = 'IGNORE'; truncate($f, 0); \
         print $f \"{kind} $$ - 0 0\\n\"; $f->flush; print \"held\\n\"; sleep 60"
    );
    let mut child = Command::new("perl")
        .args(["-e", &script])
        .arg(dir.join("postal.lock"))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line, "held\n");
    child
}

fn alive(child: &mut Child) -> bool {
    child.try_wait().unwrap().is_none()
}

#[test]
fn a_tui_that_does_not_answer_is_offered_to_be_forced_out() {
    let (imap, smtp, editor) = (inbox(), FakeSmtp::start(), Editor::new());
    let home = home(&imap, &smtp);
    let mut stuck = holder(&home, "tui");

    let mut second = postal(&home, &editor.command());
    moved_here(&mut second);
    let offered = |s: &Screen| s.contains("is not responding") && s.contains("Force quit it?");
    second
        .wait_until_for(offered, Duration::from_secs(15))
        .unwrap();
    assert!(alive(&mut stuck));
    second.send(Key::Char('y')).unwrap();
    second.wait_until(listed).unwrap();
    assert!(!alive(&mut stuck));
    quit(second);
}

// A folder sync may run inside a long-lived `postal mcp`, which a signal
// would kill whole.
#[test]
fn a_folder_sync_holding_the_lock_is_waited_for_and_never_signalled() {
    let (imap, smtp, editor) = (inbox(), FakeSmtp::start(), Editor::new());
    let home = home(&imap, &smtp);
    let mut sync = holder(&home, "sync");

    let mut second = postal(&home, &editor.command());
    second
        .wait_until(|s| s.contains("Waiting for a folder sync to finish"))
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(alive(&mut sync));
    sync.kill().unwrap();
    sync.wait().unwrap();
    second.wait_until(listed).unwrap();
    quit(second);
}

// A SIGTERM from elsewhere, a shutdown or `kill`, must not claim a move.
#[test]
fn a_sigterm_no_takeover_sent_quits_without_naming_a_terminal() {
    let (imap, smtp, editor) = (inbox(), FakeSmtp::start(), Editor::new());
    let home = home(&imap, &smtp);
    let mut first = postal(&home, &editor.command());
    first.wait_until(listed).unwrap();
    first.signal(Signal::Term).unwrap();
    let text = left(&mut first);
    assert!(text.contains("postal: quit on SIGTERM"), "{text}");
    assert!(!text.contains("moved"), "{text}");
}
