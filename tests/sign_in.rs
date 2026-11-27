//! `postal` runs the login flow itself when it has no Account to sign in
//! as, and `L` signs in again once the server stops accepting the Account.

use crate::support;

use std::{
    fs,
    time::{Duration, Instant},
};

use support::{
    browser::{Browser, follow_redirect},
    fake_imap::{EMAIL, FakeImap, FakeMessage, Outage},
    fake_oauth::{CODE, FakeOAuth, REFRESH_TOKEN},
    home::Home,
    query::{self, Form},
    wait_for,
};
use termlens::{Key, Terminal};

fn spawn(home: &Home, browser: &Browser, args: &[&str]) -> Terminal {
    // Tall enough for the `?` popup to reach the App group, which lists `L`.
    home.terminal()
        .size(100, 36)
        .env("PATH", browser.path())
        .args(args)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn hello() -> Vec<FakeMessage> {
    vec![FakeMessage::new("Alice", "Hello", "2024-03-02T09:00:00Z")]
}

/// The consent URL Postal opened, once it has.
fn consent(browser: &Browser) -> Form {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(url) = browser.opened().pop() {
            return query::of_url(&url);
        }

        assert!(Instant::now() < deadline, "no browser opened");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Plays the user granting access: Google redirects with the code.
fn grant(consent: &Form) {
    let code = CODE.replace('/', "%2F");
    let granted = format!("code={code}&state={}&scope=email", consent["state"]);
    assert!(follow_redirect(&consent["redirect_uri"], &granted).contains("200"));
}

#[test]
fn a_first_run_signs_in_then_opens_the_inbox() {
    let server = FakeImap::start(hello());
    let oauth = FakeOAuth::start();
    // No Account yet: only what a test must point at the fakes.
    let home = Home::new(&format!(
        "[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        server.port,
        oauth.config()
    ));
    fs::remove_file(home.secret_path()).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &[]);
    t.wait_until(|s| s.contains("Google (Gmail)")).unwrap();
    t.send(Key::Enter).unwrap();

    grant(&consent(&browser));
    t.wait_until(|s| s.contains("Hello")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains(&format!("email = \"{EMAIL}\"")), "{config}");
}

#[test]
fn a_missing_secret_signs_in_again_as_the_account() {
    let server = FakeImap::start(hello());
    let home = Home::for_server(&server);
    fs::remove_file(home.secret_path()).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &[]);

    let consent = consent(&browser);
    assert_eq!(consent["login_hint"], EMAIL);
    grant(&consent);
    t.wait_until(|s| s.contains("Hello")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// Plays the server refusing the Account mid-session: Postal shows it
/// signed out, with the Inbox it had.
fn sign_out(t: &mut Terminal, server: &FakeImap) {
    t.wait_until(|s| s.contains("Hello")).unwrap();
    server.fail(Outage::Revoked);
    t.wait_until(|s| s.contains("signed out · L sign in"))
        .unwrap();
    assert!(t.screen().contains("Hello"));
}

/// Grants the consent `L` opens, as the Account it had.
fn sign_in_again(t: &mut Terminal, browser: &Browser) {
    t.send(Key::Char('L')).unwrap();
    grant(&consent(browser));
}

#[test]
fn l_signs_in_again_and_restarts_sync() {
    let server = FakeImap::start(hello());
    let home = Home::for_server(&server);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &[]);
    t.wait_until(|s| s.contains("Hello")).unwrap();
    // `L` is neither active nor listed while signed in.
    t.send(Key::Char('?')).unwrap();
    t.send_str("/sign in").unwrap();
    t.wait_until(|s| s.contains("no key matches")).unwrap();
    t.send(Key::Enter).unwrap();
    t.send(Key::Char('?')).unwrap();
    t.send(Key::Char('L')).unwrap();
    // The popup opening proves `L` left the TUI where it was.
    t.send(Key::Char('?')).unwrap();
    t.wait_until(|s| s.contains(" Help ")).unwrap();
    t.send(Key::Char('?')).unwrap();
    t.wait_until(|s| !s.contains(" Help ")).unwrap();

    sign_out(&mut t, &server);
    t.send(Key::Char('?')).unwrap();
    t.send_str("/sign in").unwrap();
    t.wait_until(|s| s.contains("sign in again")).unwrap();
    t.send(Key::Enter).unwrap();
    t.send(Key::Char('?')).unwrap();
    t.wait_until(|s| !s.contains(" Help ")).unwrap();
    server.recover();
    sign_in_again(&mut t, &browser);

    t.wait_until(|s| s.contains("Hello") && !s.contains("signed out"))
        .unwrap();
    server.deliver(FakeMessage::new(
        "Bob",
        "Welcome back",
        "2024-03-03T09:00:00Z",
    ));
    t.wait_until(|s| s.contains("Welcome back")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

// Ctrl-C ends the sign-in alone, never the TUI it was opened from.
#[test]
fn ctrl_c_in_the_login_flow_returns_to_the_tui_signed_out() {
    let server = FakeImap::start(hello());
    let home = Home::for_server(&server);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &[]);
    sign_out(&mut t, &server);
    server.recover();
    t.send(Key::Char('L')).unwrap();
    consent(&browser);

    t.send(Key::Ctrl('c')).unwrap();
    t.wait_until(|s| s.contains("Hello") && s.contains("signed out · L sign in"))
        .unwrap();
    assert_eq!(
        fs::read_to_string(home.secret_path()).unwrap(),
        format!("{REFRESH_TOKEN}\n")
    );
    // Signed out stays so: nothing reconnects behind the screen.
    server.clear_commands();
    std::thread::sleep(Duration::from_millis(300));
    assert!(server.commands().is_empty(), "{:?}", server.commands());
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

/// An Account signed in with OAuth: its secret is the refresh token.
fn oauth_account(server: &FakeImap, oauth: &FakeOAuth) -> Home {
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        server.port,
        oauth.config()
    ));
    fs::write(home.secret_path(), format!("{REFRESH_TOKEN}\n")).unwrap();
    home
}

// A refused access token is no outage: retrying it would only be refused again.
#[test]
fn a_refused_account_shows_signed_out_in_red_and_lists_l() {
    let server = FakeImap::start(hello());
    let home = Home::for_server(&server);
    server.fail(Outage::Revoked);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &[]);

    let status = "signed out · L sign in";
    let screen = t.snapshot_after(|s| s.contains(status)).unwrap();
    let hints = screen.row_text(screen.rows() - 1);
    let column = u16::try_from(hints.split_once(status).unwrap().0.chars().count()).unwrap();
    let cell = screen.cell(screen.rows() - 1, column).unwrap();
    assert_eq!(cell.style().fg, termlens::Color::Indexed(1), "{hints}");
    assert_eq!(hints.matches("L sign in").count(), 2, "{hints}");
    // Logged once both connections have ended: a retry would come before it.
    let log = || fs::read_to_string(home.log_path()).unwrap_or_default();
    wait_for(|| log().contains(&format!("connection::serve: {EMAIL}: ")));
    assert!(!log().contains("retrying in"), "{}", log());
    let logins = server
        .commands()
        .iter()
        .filter(|c| c.starts_with("AUTHENTICATE"))
        .count();
    // One each for the sync and the body connections, and no retry.
    assert!(logins <= 2, "{:?}", server.commands());
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn a_refresh_token_revoked_mid_session_signs_out_and_l_offers_the_account() {
    let server = FakeImap::start(hello());
    let oauth = FakeOAuth::start();
    // Each connection then refreshes the access token first.
    oauth.expire_in(30);
    let home = oauth_account(&server, &oauth);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &[]);
    t.wait_until(|s| s.contains("Hello")).unwrap();

    oauth.revoke();
    server.fail(Outage::Offline);
    server.recover();
    t.wait_until(|s| s.contains("signed out · L sign in"))
        .unwrap();
    t.send(Key::Char('L')).unwrap();
    assert_eq!(consent(&browser)["login_hint"], EMAIL);
    t.send(Key::Ctrl('c')).unwrap();
    t.wait_until(|s| s.contains("Hello") && s.contains("signed out"))
        .unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}
