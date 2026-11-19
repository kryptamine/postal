//! `postal login` signs in through Google's consent page, and a Postal
//! signed in that way trades its refresh token for an access token at start.

mod support;

use std::{
    fs,
    time::{Duration, Instant},
};

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use support::{
    browser::{Browser, follow_redirect},
    fake_imap::{ACCESS_TOKEN, EMAIL, FakeImap, xoauth2},
    fake_oauth::{CODE, FakeOAuth, REFRESH_TOKEN},
    home::Home,
    query::{self, Form},
};
use termlens::{Key, Terminal};

/// The `[oauth]` overrides that point Postal at `oauth`.
fn overrides(oauth: &FakeOAuth) -> String {
    format!(
        "\n[oauth]\nauthorization-url = \"http://127.0.0.1:{}/auth\"\ntoken-url = \"{}\"\n",
        oauth.port,
        oauth.token_url()
    )
}

/// An Account signed in with OAuth: its secret is the refresh token.
fn signed_in(server: &FakeImap, oauth: &FakeOAuth) -> Home {
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\nauth = \"oauth\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
        server.port,
        overrides(oauth)
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
        overrides(oauth)
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
fn assert_failed_writing_nothing(t: &mut Terminal, home: &Home, seed: &str, message: &str) {
    let screen = t.snapshot_after(|s| s.contains("postal:")).unwrap();
    assert!(!t.wait_exit().unwrap().success(), "{screen:?}");
    assert!(screen.logical_text().contains(message), "{screen:?}");
    assert_eq!(secret(home), None);
    assert_eq!(fs::read_to_string(home.config_path()).unwrap(), seed);
}

fn spawn(home: &Home, browser: &Browser, args: &[&str]) -> Terminal {
    Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("PATH", browser.path())
        .env("TZ", "UTC")
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
    t.wait_until(|s| s.contains(" unread ")).unwrap();
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
    t.wait_until(|s| s.contains(" unread ")).unwrap();
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
    t.wait_until(|s| s.contains(" unread ")).unwrap();
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
    let url = consent_url(&browser);
    t.wait_until(|s| s.logical_text().contains(&url)).unwrap();
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
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains(&format!("email = \"{EMAIL}\"")), "{config}");
    assert!(config.contains("auth = \"oauth\""), "{config}");
    let commands = server.commands();
    let authenticate = commands.iter().position(|c| c == "AUTHENTICATE XOAUTH2");
    let examine = commands.iter().position(|c| c.starts_with("EXAMINE"));
    assert!(
        authenticate.is_some() && authenticate < examine,
        "{commands:?}"
    );

    // The written config must be one Postal starts from.
    let mut t = spawn(&home, &browser, &[]);
    t.wait_until(|s| s.contains(" unread ")).unwrap();
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

#[test]
fn a_denied_consent_suggests_the_app_password() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded(&server, &oauth);
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
    let consent = query::of_url(&consent_url(&browser));
    let denied = format!("error=access_denied&state={}", consent["state"]);
    follow_redirect(&consent["redirect_uri"], &denied);

    assert_failed_writing_nothing(&mut t, &home, &seed, "postal login --app-password");
    assert!(oauth.requests().is_empty());
}

#[test]
fn a_browser_that_never_comes_back_times_out() {
    let server = FakeImap::start(Vec::new());
    let oauth = FakeOAuth::start();
    let home = seeded_with(&server, &oauth, "redirect-timeout = 1\n");
    let seed = fs::read_to_string(home.config_path()).unwrap();
    let browser = Browser::new();
    let mut t = spawn(&home, &browser, &["login"]);
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
    grant(&query::of_url(&consent_url(&browser)));
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let mut t = spawn(&home, &browser, &["-vv"]);
    t.wait_until(|s| s.contains(" unread ")).unwrap();
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
