//! An iCloud Account signs in with an app-specific password, checked by
//! `LOGIN` on the IMAP server before anything is written, and sends with it
//! over SMTP. The fake serves iCloud's capabilities and Folder names.

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
    browser::Browser,
    fake_imap::{APP_PASSWORD, EMAIL, FakeImap, FakeMessage, LOGIN_USER},
    fake_smtp::FakeSmtp,
    home::Home,
};
use tempfile::TempDir;
use termlens::{Key, Terminal};

const WRITTEN: &str = "To: Bob Example <bob@example.com>\nSubject: Plans\n\nHi Bob\n";

/// The `[imap]` and `[smtp]` tables pointing at the fakes, and the file store.
fn servers(imap: &FakeImap, smtp_port: u16, smtp_tls: &str) -> String {
    format!(
        "[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n\
         [smtp]\nhost = \"127.0.0.1\"\nport = {smtp_port}\n{smtp_tls}\n\
         [secrets]\nstore = \"file\"\n",
        imap.port
    )
}

/// A config with no Account yet.
fn seeded(imap: &FakeImap) -> Home {
    let home = Home::new(&servers(imap, 1, "tls = false\n"));
    fs::remove_file(home.secret_path()).unwrap();
    home
}

/// An iCloud Account signed in already, sending to `smtp_port`.
fn signed_in(imap: &FakeImap, smtp_port: u16, smtp_tls: &str) -> Home {
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\nprovider = \"icloud\"\n\n{}",
        servers(imap, smtp_port, smtp_tls)
    ));
    fs::write(home.secret_path(), format!("{APP_PASSWORD}\n")).unwrap();
    home
}

/// An iCloud-like server holding one Message in the Inbox.
fn icloud() -> FakeImap {
    let server = FakeImap::start(vec![FakeMessage::new(
        "Ada Lovelace",
        "Weekly notes",
        "2024-11-18T09:00:00Z",
    )]);
    server.as_icloud();
    server
}

/// `$EDITOR`, copying `WRITTEN` over the Draft.
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

    fn command(&self) -> String {
        format!("cp '{}'", self.dir.path().join("next").display())
    }
}

fn spawn(home: &Home, browser: &Browser, editor: &Editor, args: &[&str]) -> Terminal {
    Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("PATH", browser.path())
        .env("EDITOR", editor.command())
        .env("TZ", "UTC")
        .args(args)
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[track_caller]
fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "timed out at {}",
            std::panic::Location::caller()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn secret(home: &Home) -> Option<String> {
    let text = fs::read_to_string(home.secret_path()).ok()?;
    Some(text.trim_end().to_owned())
}

/// Picks iCloud, second on the Provider list.
fn choose_icloud(t: &mut Terminal) {
    t.wait_until(|s| s.contains("Google (Gmail)") && s.contains("iCloud"))
        .unwrap();
    t.send(Key::Down).unwrap();
    t.send(Key::Enter).unwrap();
}

/// Types `password` into the masked field and signs in with it.
fn type_password(t: &mut Terminal, password: &str) {
    t.wait_until(|s| s.contains("App-specific password:"))
        .unwrap();
    t.send_str(password).unwrap();
    let dots = "•".repeat(password.chars().count());
    let screen = t.snapshot_after(|s| s.contains(&dots)).unwrap();
    assert!(!screen.contains(password), "{screen:?}");
    t.send(Key::Enter).unwrap();
}

/// The `LOGIN` commands the server got, by user name.
fn logins(server: &FakeImap) -> Vec<String> {
    server
        .commands()
        .iter()
        .filter_map(|c| c.strip_prefix("LOGIN "))
        .map(|rest| rest.split(' ').next().unwrap().trim_matches('"').to_owned())
        .collect()
}

#[test]
fn an_app_specific_password_signs_in_and_is_written() {
    let server = icloud();
    let home = seeded(&server);
    let (browser, editor) = (Browser::new(), Editor::new());
    let mut t = spawn(&home, &browser, &editor, &["login"]);
    choose_icloud(&mut t);
    // `q` and `j` type into a field rather than quit or move.
    t.wait_until(|s| s.contains("Your iCloud address:"))
        .unwrap();
    t.send_str("qj").unwrap();
    t.wait_until(|s| s.contains("qj")).unwrap();
    t.send(Key::Backspace).unwrap();
    t.send(Key::Backspace).unwrap();
    t.send_str(EMAIL).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(&format!("Sign in to iCloud as {EMAIL}")))
        .unwrap();
    type_password(&mut t, APP_PASSWORD);

    t.wait_until(|s| s.contains(&format!("Signed in as {EMAIL}.")))
        .unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert!(browser.opened().is_empty());
    assert_eq!(secret(&home).as_deref(), Some(APP_PASSWORD));
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains("provider = \"icloud\"\n"), "{config}");
    assert_eq!(logins(&server), [LOGIN_USER]);
    let commands = server.commands();
    assert!(
        !commands.iter().any(|c| c.starts_with("AUTHENTICATE")),
        "{commands:?}"
    );

    // The written config must be one Postal starts from, signing in the same way.
    server.clear_commands();
    let mut t = spawn(&home, &browser, &editor, &[]);
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    quit(t);
    assert!(logins(&server).iter().all(|user| user == LOGIN_USER));
}

// iCloud wants the whole address for a custom domain.
#[test]
fn the_whole_address_signs_in_where_the_name_part_is_refused() {
    let server = icloud();
    server.login_as(EMAIL);
    let home = seeded(&server);
    let (browser, editor) = (Browser::new(), Editor::new());
    let mut t = spawn(&home, &browser, &editor, &["login"]);
    choose_icloud(&mut t);
    t.wait_until(|s| s.contains("Your iCloud address:"))
        .unwrap();
    t.send_str(EMAIL).unwrap();
    t.send(Key::Enter).unwrap();
    type_password(&mut t, APP_PASSWORD);

    t.wait_until(|s| s.contains(&format!("Signed in as {EMAIL}.")))
        .unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(logins(&server), [LOGIN_USER, EMAIL]);
}

#[test]
fn a_refused_password_writes_nothing_and_keeps_the_address_for_the_next_try() {
    let server = icloud();
    let home = seeded(&server);
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let (browser, editor) = (Browser::new(), Editor::new());
    let mut t = spawn(&home, &browser, &editor, &["login"]);
    choose_icloud(&mut t);
    t.wait_until(|s| s.contains("Your iCloud address:"))
        .unwrap();
    t.send_str(EMAIL).unwrap();
    t.send(Key::Enter).unwrap();
    type_password(&mut t, "wrong-password");

    t.wait_until(|s| {
        s.contains("Sign-in failed")
            && s.contains("refused the address or the app-specific password")
    })
    .unwrap();
    assert_eq!(secret(&home), None);
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), seed);

    t.send(Key::Enter).unwrap();
    choose_icloud(&mut t);
    t.wait_until(|s| s.contains("Your iCloud address:") && s.contains(EMAIL))
        .unwrap();
    t.send(Key::Enter).unwrap();
    type_password(&mut t, APP_PASSWORD);
    t.wait_until(|s| s.contains(&format!("Signed in as {EMAIL}.")))
        .unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some(APP_PASSWORD));
}

#[test]
fn a_lost_secret_asks_for_the_password_of_the_account_alone() {
    let server = icloud();
    let home = signed_in(&server, 1, "tls = false\n");
    fs::remove_file(home.secret_path()).unwrap();
    let (browser, editor) = (Browser::new(), Editor::new());
    let mut t = spawn(&home, &browser, &editor, &[]);
    t.wait_until(|s| s.contains(&format!("Sign in to iCloud as {EMAIL}")))
        .unwrap();
    type_password(&mut t, APP_PASSWORD);

    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    quit(t);
    assert_eq!(secret(&home).as_deref(), Some(APP_PASSWORD));
    assert!(
        server
            .commands()
            .iter()
            .all(|c| !c.starts_with("AUTHENTICATE"))
    );
}

#[test]
fn a_password_never_reaches_the_log() {
    let server = icloud();
    let home = seeded(&server);
    let (browser, editor) = (Browser::new(), Editor::new());
    let mut t = spawn(&home, &browser, &editor, &["login", "-vv"]);
    choose_icloud(&mut t);
    t.wait_until(|s| s.contains("Your iCloud address:"))
        .unwrap();
    t.send_str(EMAIL).unwrap();
    t.send(Key::Enter).unwrap();
    type_password(&mut t, APP_PASSWORD);
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let mut t = spawn(&home, &browser, &editor, &["-vv"]);
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    quit(t);

    let login = home.log_path().with_extension("log.1");
    let log = fs::read_to_string(&login).unwrap() + &fs::read_to_string(home.log_path()).unwrap();
    assert!(log.contains(" C: "), "no wire trace: {log}");
    assert!(!log.contains(APP_PASSWORD), "logged the password: {log}");
}

#[test]
fn a_message_goes_out_with_the_password_and_is_saved_to_sent_messages() {
    let (server, smtp) = (icloud(), FakeSmtp::start());
    let home = signed_in(&server, smtp.port, "tls = false\n");
    let (browser, editor) = (Browser::new(), Editor::new());
    let mut t = spawn(&home, &browser, &editor, &[]);
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains(" Draft ") && s.contains("y send"))
        .unwrap();
    t.send(Key::Char('y')).unwrap();

    wait_for(|| !server.appended().is_empty());
    quit(t);
    let logins = smtp.logins();
    let plain = format!("\0{EMAIL}\0{APP_PASSWORD}");
    assert_eq!(logins, [("PLAIN".to_owned(), plain)]);
    assert_eq!(smtp.received().len(), 1);
    let appended = server.appended();
    assert_eq!(appended.len(), 1);
    assert_eq!(appended[0].0, "Sent Messages");
}

// iCloud's SMTP takes no implicit TLS, and the password must never go out
// before the connection is encrypted.
#[test]
fn smtp_asks_for_starttls_before_it_signs_in() {
    let (server, smtp) = (icloud(), FakeSmtp::start());
    let home = signed_in(&server, smtp.port, "tls = true\nstarttls = true\n");
    let (browser, editor) = (Browser::new(), Editor::new());
    let mut t = spawn(&home, &browser, &editor, &[]);
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains(" Draft ") && s.contains("y send"))
        .unwrap();
    t.send(Key::Char('y')).unwrap();

    wait_for(|| smtp.starttls() > 0);
    quit(t);
    assert!(smtp.logins().is_empty(), "{:?}", smtp.logins());
    assert!(smtp.received().is_empty());
}

// iCloud lists no SPECIAL-USE: its Trash is known by the name it gives it.
#[test]
fn a_delete_moves_to_deleted_messages_without_move() {
    let server = icloud();
    let home = signed_in(&server, 1, "tls = false\n");
    let (browser, editor) = (Browser::new(), Editor::new());
    let mut t = spawn(&home, &browser, &editor, &[]);
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    wait_for(|| server.commands().iter().any(|c| c == "IDLE"));
    t.send(Key::Backspace).unwrap();

    wait_for(|| server.subjects("Deleted Messages") == ["Weekly notes"]);
    quit(t);
    let uid = support::fake_imap::FIRST_UID;
    let commands = server.commands();
    assert!(
        commands.contains(&format!("UID COPY {uid} \"Deleted Messages\"")),
        "{commands:#?}"
    );
}

// A server without IDLE is asked by NOOP, which wakes for the screen.
#[test]
fn without_idle_a_noop_takes_in_new_mail() {
    let server = icloud();
    server.without_idle();
    let home = signed_in(&server, 1, "tls = false\n");
    let (browser, editor) = (Browser::new(), Editor::new());
    let mut t = spawn(&home, &browser, &editor, &[]);
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    server.deliver(FakeMessage::new("Bob", "Lunch", "2024-11-19T09:00:00Z"));
    // Marking unread asks the sync connection, which ends its wait.
    t.send(Key::Char('U')).unwrap();

    t.wait_until(|s| s.contains("Lunch")).unwrap();
    quit(t);
    let commands = server.commands();
    assert!(commands.iter().any(|c| c == "NOOP"), "{commands:#?}");
    assert!(!commands.iter().any(|c| c == "IDLE"), "{commands:#?}");
}
