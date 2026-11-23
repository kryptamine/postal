//! `postal login` signs in through Google's consent page, and a Postal
//! signed in that way trades its refresh token for an access token at start.

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
    time::{Duration, Instant},
};

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use support::{
    browser::{Browser, follow_redirect},
    fake_imap::{ACCESS_TOKEN, EMAIL, FakeImap, Outage, xoauth2},
    fake_oauth::{CODE, FakeOAuth, REFRESH_TOKEN},
    home::Home,
    query::{self, Form},
};
use termlens::{Key, Terminal};

/// An Account signed in with OAuth: its secret is the refresh token.
fn signed_in(server: &FakeImap, oauth: &FakeOAuth) -> Home {
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        server.port,
        oauth.config()
    ));
    fs::write(home.secret_path(), format!("{REFRESH_TOKEN}\n")).unwrap();
    home
}

/// A config with no Account yet: the fake servers and the file store.
fn seeded(server: &FakeImap, oauth: &FakeOAuth) -> Home {
    seeded_with(server, oauth, "")
}

/// As `seeded`, with `oauth_extra` added to the `[oauth]` table.
fn seeded_with(server: &FakeImap, oauth: &FakeOAuth, oauth_extra: &str) -> Home {
    let home = Home::new(&format!(
        "[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}{oauth_extra}",
        server.port,
        oauth.config()
    ));
    fs::remove_file(home.secret_path()).unwrap();
    home
}

/// The consent URL Postal opened, once it has.
fn consent_url(browser: &Browser) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(url) = browser.opened().pop() {
            return url;
        }

        assert!(Instant::now() < deadline, "no browser opened");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Plays the user granting access: Google redirects with the code.
fn grant(consent: &Form) -> String {
    let code = CODE.replace('/', "%2F");
    let granted = format!("code={code}&state={}&scope=email", consent["state"]);
    follow_redirect(&consent["redirect_uri"], &granted)
}

/// The secret file without its line break.
fn secret(home: &Home) -> Option<String> {
    let text = fs::read_to_string(home.secret_path()).ok()?;
    Some(text.trim_end().to_owned())
}

/// Asserts the login failed with `message` and left the seeded config alone.
/// Quits the failure Postal shows, then checks it exits with `message`.
fn assert_failed_writing_nothing(t: &mut Terminal, home: &Home, seed: &str, message: &str) {
    t.wait_until(|s| s.contains("Sign-in failed")).unwrap();
    t.send(Key::Char('q')).unwrap();
    let screen = t.snapshot_after(|s| s.contains("postal:")).unwrap();
    assert!(!t.wait_exit().unwrap().success(), "{screen:?}");
    assert!(screen.logical_text().contains(message), "{screen:?}");
    assert_eq!(secret(home), None);
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), seed);
}

/// Picks the one Provider the list offers.
fn choose_google(t: &mut Terminal) {
    t.wait_until(|s| s.contains("Google (Gmail)")).unwrap();
    t.send(Key::Enter).unwrap();
}

/// The consent URLs Postal opened, once there are `count`.
fn consent_urls(browser: &Browser, count: usize) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let opened = browser.opened();
        if opened.len() >= count {
            return opened;
        }

        assert!(Instant::now() < deadline, "opened only {opened:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn spawn(home: &Home, browser: &Browser, args: &[&str]) -> Terminal {
    spawn_with(home, browser, args, &[])
}

fn spawn_with(home: &Home, browser: &Browser, args: &[&str], envs: &[(&str, &str)]) -> Terminal {
    Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("PATH", browser.path())
        .env("TZ", "UTC")
        .envs(envs.iter().copied())
        .args(args)
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

#[test]
fn postal_refreshes_the_access_token_at_start() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = signed_in(&server, &oauth);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &[]);
    t.wait_until(|s| s.contains("Inbox is empty")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let requests = oauth.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0]["grant_type"], "refresh_token");
    assert_eq!(requests[0]["refresh_token"], REFRESH_TOKEN);
    let commands = server.commands();
    assert!(
        commands.iter().any(|c| c == "AUTHENTICATE XOAUTH2"),
        "{commands:?}"
    );
    assert!(
        !commands.iter().any(|c| c.starts_with("LOGIN")),
        "{commands:?}"
    );
}

#[test]
fn an_unreachable_token_endpoint_at_start_is_an_outage() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = signed_in(&server, &oauth);
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = format!("http://{}/token", closed.local_addr().unwrap());
    drop(closed);
    let config = fs::read_to_string(home.config_path()).unwrap();
    fs::write(
        home.config_path(),
        config.replace(&oauth.token_url(), &dead),
    )
    .unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &[]);
    t.wait_until(|s| s.contains("offline · retry in")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn a_revoked_refresh_token_at_start_signs_in_again_as_the_account() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = signed_in(&server, &oauth);
    fs::write(home.secret_path(), "1//revoked\n").unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &[]);
    let consent = query::of_url(&consent_url(&browser));
    assert_eq!(consent["login_hint"], EMAIL);
    assert!(server.commands().is_empty(), "{:?}", server.commands());

    grant(&consent);
    t.wait_until(|s| s.contains("Inbox is empty")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some(REFRESH_TOKEN));
}

#[test]
fn an_access_token_near_its_expiry_is_refreshed_before_a_connection() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    oauth.expire_in(30);
    let home = signed_in(&server, &oauth);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &[]);
    t.wait_until(|s| s.contains("Inbox is empty")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    // One at start, then one more for the connection that synced.
    let requests = oauth.requests();
    assert!(requests.len() >= 2, "{requests:?}");
    assert!(requests.iter().all(|r| r["grant_type"] == "refresh_token"));
}

#[test]
fn a_granted_consent_writes_config_and_refresh_token() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    let url = consent_url(&browser);
    // Without a browser that opened, the URL is the way to sign in.
    t.wait_until(|s| s.contains("If it did not open")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.clipboard().and_then(|c| c.text()) == Some(url.as_str()))
        .unwrap();
    assert!(
        url.starts_with(&format!("http://127.0.0.1:{}/auth?", oauth.port)),
        "{url}"
    );
    let consent = query::of_url(&url);
    assert_eq!(consent["scope"], "https://mail.google.com/ openid email");
    assert_eq!(consent["code_challenge_method"], "S256");
    assert!(
        consent["redirect_uri"].starts_with("http://127.0.0.1:"),
        "{url}"
    );

    // A browser may open a socket ahead of need and send nothing on it.
    let host = consent["redirect_uri"].strip_prefix("http://").unwrap();
    let _idle = std::net::TcpStream::connect(host).unwrap();

    // Another page cannot hand Postal a code without the state it sent.
    let forged = format!("code=forged&state=not-{}", consent["state"]);
    let status = follow_redirect(&consent["redirect_uri"], &forged);
    assert!(status.contains("404"), "{status}");

    assert!(grant(&consent).contains("200"));
    t.wait_until(|s| s.contains(&format!("Signed in as {EMAIL}.")))
        .unwrap();
    assert!(t.wait_exit().unwrap().success());

    let requests = oauth.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    let exchange = &requests[0];
    assert_eq!(exchange["grant_type"], "authorization_code");
    assert_eq!(exchange["code"], CODE);
    assert_eq!(exchange["redirect_uri"], consent["redirect_uri"]);
    let digest = ring::digest::digest(&ring::digest::SHA256, exchange["code_verifier"].as_bytes());
    assert_eq!(URL_SAFE_NO_PAD.encode(digest), consent["code_challenge"]);

    assert_eq!(secret(&home).as_deref(), Some(REFRESH_TOKEN));
    let mode = fs::metadata(home.secret_path())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains(&format!("email = \"{EMAIL}\"")), "{config}");
    let commands = server.commands();
    let authenticate = commands.iter().position(|c| c == "AUTHENTICATE XOAUTH2");
    let examine = commands.iter().position(|c| c.starts_with("EXAMINE"));
    assert!(
        authenticate.is_some() && authenticate < examine,
        "{commands:?}"
    );

    // The written config must be one Postal starts from.
    let mut t = spawn(&home, &browser, &[]);
    t.wait_until(|s| s.contains("Inbox is empty")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn a_denied_consent_names_the_refusal() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    let consent = query::of_url(&consent_url(&browser));
    let denied = format!("error=access_denied&state={}", consent["state"]);
    follow_redirect(&consent["redirect_uri"], &denied);

    assert_failed_writing_nothing(&mut t, &home, &seed, "refused the sign-in: access_denied");
    assert_eq!(
        oauth.requests(),
        Vec::<std::collections::HashMap<String, String>>::new()
    );
}

#[test]
fn a_browser_that_never_comes_back_times_out() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded_with(&server, &oauth, "redirect-timeout = 1\n");
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    consent_url(&browser);

    assert_failed_writing_nothing(&mut t, &home, &seed, "did not come back within 1s");
}

// Google's consent screen lets the user untick Gmail and still grant the rest.
#[test]
fn a_consent_without_the_gmail_scope_fails_clearly() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    oauth.grant_scope("openid https://www.googleapis.com/auth/userinfo.email");
    let home = seeded(&server, &oauth);
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    grant(&query::of_url(&consent_url(&browser)));

    assert_failed_writing_nothing(&mut t, &home, &seed, "did not grant access to Gmail");
    assert!(server.commands().is_empty(), "{:?}", server.commands());
}

#[test]
fn tokens_never_reach_the_log() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login", "-vv"]);
    choose_google(&mut t);
    grant(&query::of_url(&consent_url(&browser)));
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let mut t = spawn(&home, &browser, &["-vv"]);
    t.wait_until(|s| s.contains("Inbox is empty")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let login = home.log_path().with_extension("log.1");
    let log = fs::read_to_string(&login).unwrap() + &fs::read_to_string(home.log_path()).unwrap();
    assert!(log.contains(" C: "), "no wire trace: {log}");
    let secrets = [
        CODE,
        REFRESH_TOKEN,
        ACCESS_TOKEN,
        &STANDARD.encode(xoauth2()),
    ];
    for secret in secrets {
        assert!(!log.contains(secret), "logged {secret}: {log}");
    }
}

// A login adds the Account's keys and leaves the rest of the file as written.
#[test]
fn a_login_keeps_the_comments_and_the_layout() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let seed = fs::read_to_string(home.config_path())
        .unwrap()
        .replace("tls = false\n", "tls = false # loopback\n");
    let seed = format!("# hand-written\n{seed}");
    fs::write(home.config_path(), &seed).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    grant(&query::of_url(&consent_url(&browser)));
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let config = fs::read_to_string(home.config_path()).unwrap();
    assert_eq!(config, format!("email = \"{EMAIL}\"\n\n{seed}"));
}

// Accounts written inline become tables, the only form a new one has.
#[test]
fn a_login_adds_an_account_beside_inline_ones() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let inline = "account = [{ email = \"old@example.com\" }]\n\n";
    fs::write(home.config_path(), format!("{inline}{seed}")).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    grant(&query::of_url(&consent_url(&browser)));
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(
        config.contains("[[account]]\nemail = \"old@example.com\"\n"),
        "{config}"
    );
    assert!(
        config.contains(&format!("[[account]]\nemail = \"{EMAIL}\"\n")),
        "{config}"
    );
}

// What the config set before a login must stay.
#[test]
fn a_login_keeps_the_name_the_icons_and_the_smtp_server() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let extra = "name = \"Zoë Example\"\nicons = \"nerd\"\n\n[smtp]\nhost = \"127.0.0.1\"\nport = 2525\ntls = false\n";
    let config = fs::read_to_string(home.config_path()).unwrap();
    fs::write(home.config_path(), format!("{extra}{config}")).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    grant(&query::of_url(&consent_url(&browser)));
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains("name = \"Zoë Example\"\n"), "{config}");
    assert!(config.contains("icons = \"nerd\"\n"), "{config}");
    assert!(
        config.contains("[smtp]\nhost = \"127.0.0.1\"\nport = 2525\ntls = false\n"),
        "{config}"
    );
}

#[test]
fn a_dropped_connection_exits_non_zero_and_writes_nothing() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    let consent = query::of_url(&consent_url(&browser));

    server.fail(Outage::Offline);
    grant(&consent);
    assert_failed_writing_nothing(&mut t, &home, &seed, "postal:");
}

#[test]
fn a_login_with_the_same_email_replaces_only_the_secret() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = signed_in(&server, &oauth);
    // Not a byte of the config changes.
    let config = format!(
        "# hand-written\n{}",
        fs::read_to_string(home.config_path()).unwrap()
    );
    fs::write(home.config_path(), &config).unwrap();
    fs::write(home.secret_path(), "1//revoked\n").unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    grant(&query::of_url(&consent_url(&browser)));

    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some(REFRESH_TOKEN));
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
}

/// Another address signed in before, with a secret and a Cache of its own.
fn another_account(server: &FakeImap, oauth: &FakeOAuth) -> Home {
    let home = signed_in(server, oauth);
    let config = fs::read_to_string(home.config_path())
        .unwrap()
        .replace(EMAIL, "old@example.com");
    fs::write(home.config_path(), &config).unwrap();
    fs::rename(home.secret_path(), home.secret_path_of("old@example.com")).unwrap();
    fs::create_dir_all(home.cache_path().parent().unwrap()).unwrap();
    fs::write(home.cache_path(), "old Account").unwrap();
    home
}

#[test]
fn a_login_as_another_address_adds_an_account() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = another_account(&server, &oauth);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    grant(&query::of_url(&consent_url(&browser)));

    t.wait_until(|s| s.contains(&format!("Signed in as {EMAIL}.")))
        .unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some(REFRESH_TOKEN));
    let old = fs::read_to_string(home.secret_path_of("old@example.com")).unwrap();
    assert_eq!(old.trim_end(), REFRESH_TOKEN);
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(
        config.starts_with("email = \"old@example.com\"\n"),
        "{config}"
    );
    assert!(
        config.contains(&format!("\n[[account]]\nemail = \"{EMAIL}\"\n")),
        "{config}"
    );
    assert!(home.cache_path().exists());
}

#[test]
fn a_login_keeps_what_a_first_account_table_sets_of_its_own() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let old = "\n[[account]]\nemail = \"old@example.com\"\nname = \"Old Name\"\n\n\
               [account.smtp]\nport = 2525\n";
    let seed = fs::read_to_string(home.config_path()).unwrap() + old;
    fs::write(home.config_path(), seed).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    grant(&query::of_url(&consent_url(&browser)));

    t.wait_until(|s| s.contains(&format!("Signed in as {EMAIL}.")))
        .unwrap();
    assert!(t.wait_exit().unwrap().success());
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains(old), "{config}");
    assert!(
        config.contains(&format!("\n[[account]]\nemail = \"{EMAIL}\"\n")),
        "{config}"
    );
}

/// This Account written at iCloud, with a Cache of its own.
fn at_icloud(server: &FakeImap, oauth: &FakeOAuth) -> (Home, String) {
    let home = signed_in(server, oauth);
    let config = fs::read_to_string(home.config_path()).unwrap().replace(
        &format!("email = \"{EMAIL}\"\n"),
        &format!("email = \"{EMAIL}\"\nprovider = \"icloud\"\n"),
    );
    fs::write(home.config_path(), &config).unwrap();
    fs::create_dir_all(home.cache_path().parent().unwrap()).unwrap();
    fs::write(home.cache_path(), "old Account").unwrap();
    (home, config)
}

/// Grants the consent as `EMAIL` and answers the replacement question.
fn sign_in_at_google(t: &mut Terminal, browser: &Browser, answer: &str) {
    grant(&query::of_url(&consent_url(browser)));
    t.wait_until(|s| s.contains("signed in at iCloud") && s.contains("y replace"))
        .unwrap();
    t.send_str(answer).unwrap();
}

#[test]
fn a_login_at_another_provider_replaces_the_account_and_deletes_the_cache() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let (home, _) = at_icloud(&server, &oauth);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    sign_in_at_google(&mut t, &browser, "y");

    t.wait_until(|s| s.contains(&format!("Signed in as {EMAIL}.")))
        .unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some(REFRESH_TOKEN));
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains(&format!("email = \"{EMAIL}\"")), "{config}");
    assert!(!config.contains("icloud"), "{config}");
    assert!(!home.cache_path().exists());
}

#[test]
fn a_declined_replacement_writes_nothing() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let (home, config) = at_icloud(&server, &oauth);
    fs::write(home.secret_path(), "1//old\n").unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    sign_in_at_google(&mut t, &browser, "n");

    t.wait_until(|s| s.contains("login cancelled")).unwrap();
    assert!(!t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some("1//old"));
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), config);
    assert!(home.cache_path().exists());
}

// The keychain is never swapped for the file without the user choosing it.
#[test]
fn a_locked_keychain_hints_at_the_file_store_and_writes_no_config() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let seed = format!(
        "[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n{}",
        server.port,
        oauth.config()
    );
    let home = Home::new(&seed);
    fs::remove_file(home.secret_path()).unwrap();
    let browser = Browser::new();
    let locked = [("POSTAL_KEYCHAIN_FAIL", "the test locked the keychain")];
    let mut t = spawn_with(&home, &browser, &["login"], &locked);
    choose_google(&mut t);
    grant(&query::of_url(&consent_url(&browser)));

    t.wait_until(|s| s.contains("Sign-in failed")).unwrap();
    t.send(Key::Char('q')).unwrap();
    t.wait_until(|s| s.logical_text().contains("store = \"file\""))
        .unwrap();
    assert!(!t.wait_exit().unwrap().success());
    assert_eq!(secret(&home), None);
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), seed);
}

#[test]
fn quitting_the_provider_list_writes_nothing() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    t.wait_until(|s| s.contains("Google (Gmail)")).unwrap();
    t.send(Key::Char('q')).unwrap();

    t.wait_until(|s| s.contains("login cancelled")).unwrap();
    assert!(!t.wait_exit().unwrap().success());
    assert_eq!(browser.opened(), Vec::<String>::new());
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), seed);
}

// The first consent's listener is gone: only the second sign-in counts.
#[test]
fn esc_on_the_browser_step_returns_to_the_provider_list() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    consent_url(&browser);
    t.wait_until(|s| s.contains("Esc back")).unwrap();
    t.send(Key::Esc).unwrap();

    choose_google(&mut t);
    let urls = consent_urls(&browser, 2);
    grant(&query::of_url(&urls[1]));
    t.wait_until(|s| s.contains(&format!("Signed in as {EMAIL}.")))
        .unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some(REFRESH_TOKEN));
}

#[test]
fn a_failed_sign_in_is_shown_and_tried_again() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    choose_google(&mut t);
    let consent = query::of_url(&consent_url(&browser));
    let denied = format!("error=access_denied&state={}", consent["state"]);
    follow_redirect(&consent["redirect_uri"], &denied);
    t.wait_until(|s| s.contains("Sign-in failed") && s.contains("access_denied"))
        .unwrap();
    assert_eq!(secret(&home), None);

    t.send(Key::Enter).unwrap();
    choose_google(&mut t);
    grant(&query::of_url(&consent_urls(&browser, 2)[1]));
    t.wait_until(|s| s.contains(&format!("Signed in as {EMAIL}.")))
        .unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert_eq!(secret(&home).as_deref(), Some(REFRESH_TOKEN));
}
