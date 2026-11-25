//! An Account at any IMAP/SMTP server: Onboarding asks for the servers,
//! as the domain publishes them or guessed from it, and a password checked
//! by `LOGIN` before anything is written. The fake lists no extension past UIDPLUS.

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

use support::{
    client_config::{client_config, incoming, outgoing},
    editor::sending,
    fake_autoconfig::{self, FakeAutoconfig},
    fake_dns::FakeDns,
    fake_imap::{APP_PASSWORD, EMAIL, FIRST_UID, FakeImap, FakeMessage},
    fake_smtp::{FakeSmtp, SMTP_PASSWORD},
    home::Home,
};
use termlens::{Key, Terminal};

/// A server with no Gmail extensions, `MOVE`, `IDLE` or CONDSTORE, whose
/// Folders say their use by SPECIAL-USE alone, under names no list knows.
fn minimal() -> FakeImap {
    let server = FakeImap::start(vec![FakeMessage::new(
        "Ada Lovelace",
        "Weekly notes",
        "2024-11-18T09:00:00Z",
    )]);
    server.without_gmail();
    server.without_move();
    server.without_idle();
    server.add_folder("Gesendet", &["\\Sent"], Vec::new());
    server.add_folder("Papierkorb", &["\\Trash"], Vec::new());
    server.without_condstore();
    server.login_as(EMAIL);
    server
}

/// A config with no Account yet, keeping secrets in files, at a domain
/// that publishes no servers.
fn seeded() -> Home {
    seeded_at(&fake_autoconfig::nothing_published())
}

/// As `seeded`, with this `[autoconfig]` table.
fn seeded_at(autoconfig: &str) -> Home {
    let home = Home::new(&format!("[secrets]\nstore = \"file\"\n{autoconfig}"));
    fs::remove_file(home.secret_path()).unwrap();
    home
}

/// An Account at the fakes, signed in already.
fn signed_in(imap: &FakeImap, smtp_port: u16) -> Home {
    let home = Home::new(&format!(
        "[[account]]\nemail = \"{EMAIL}\"\nprovider = \"other\"\n\
         imap = {{ host = \"127.0.0.1\", port = {}, tls = false }}\n\
         smtp = {{ host = \"127.0.0.1\", port = {smtp_port}, tls = false }}\n\n\
         [secrets]\nstore = \"file\"\n",
        imap.port
    ));
    fs::write(home.secret_path(), format!("{APP_PASSWORD}\n")).unwrap();
    home
}

const WRITTEN: &str = "To: Bob Example <bob@example.com>\nSubject: Plans\n\nHi Bob\n";

/// `$EDITOR` copies `WRITTEN` over the Draft, from a file beside the config,
/// and sends it as `:PostalSend` does.
fn spawn(home: &Home, args: &[&str]) -> Terminal {
    let written = home.xdg_config_home().join("written.eml");
    fs::write(&written, WRITTEN).unwrap();
    home.terminal()
        .size(100, 30)
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("EDITOR", sending(&format!("cp '{}'", written.display())))
        .args(args)
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

/// Picks the generic Provider, last on the list, and types the address.
fn choose_other(t: &mut Terminal) {
    pick_other(t);
    t.send_str(EMAIL).unwrap();
    t.send(Key::Enter).unwrap();
}

fn pick_other(t: &mut Terminal) {
    t.wait_until(|s| s.contains("Other (IMAP)")).unwrap();
    t.send(Key::Down).unwrap();
    t.send(Key::Down).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("Your address:")).unwrap();
}

/// Replaces the focused field's text with `text`, then moves to the next.
fn retype(t: &mut Terminal, text: &str) {
    t.send(Key::Ctrl('u')).unwrap();
    t.send_str(text).unwrap();
    t.send(Key::Tab).unwrap();
}

/// The usernames and SMTP password typed into the form; blank where `None`.
#[derive(Default)]
struct SignIns<'a> {
    imap_user: Option<&'a str>,
    smtp: Option<(&'a str, &'a str)>,
}

/// Points the guessed servers at the fakes, in plaintext.
fn fill_servers(t: &mut Terminal, imap: &FakeImap, smtp_port: u16, sign_ins: &SignIns<'_>) {
    t.wait_until(|s| {
        s.contains(&format!("Servers for {EMAIL}"))
            && s.contains("imap.example.com")
            && s.contains("993")
            && s.contains("smtp.example.com")
            && s.contains("465")
    })
    .unwrap();
    retype(t, "127.0.0.1");
    retype(t, &imap.port.to_string());
    // TLS, then none, which a loopback host alone is offered.
    t.send(Key::Right).unwrap();
    t.send(Key::Tab).unwrap();
    match sign_ins.imap_user {
        Some(user) => retype(t, user),
        None => t.send(Key::Tab).unwrap(),
    }

    retype(t, "127.0.0.1");
    retype(t, &smtp_port.to_string());
    // TLS, STARTTLS, then none.
    t.send(Key::Right).unwrap();
    t.send(Key::Right).unwrap();
    t.send(Key::Tab).unwrap();
    if let Some((user, password)) = sign_ins.smtp {
        retype(t, user);
        retype(t, password);
    }

    t.wait_until(|s| s.to_string().matches("none").count() == 2)
        .unwrap();
    t.send(Key::Enter).unwrap();
}

fn type_password(t: &mut Terminal, password: &str) {
    t.wait_until(|s| s.contains(&format!("Sign in as {EMAIL}")) && s.contains("Password:"))
        .unwrap();
    t.send_str(password).unwrap();
    t.send(Key::Enter).unwrap();
}

/// Writes a Draft and sends it, from the Message List.
fn send_one(t: &mut Terminal) {
    t.send(Key::Char('c')).unwrap();
}

#[test]
fn the_servers_typed_sign_in_and_are_written_for_the_next_run() {
    let (server, smtp) = (minimal(), FakeSmtp::start());
    let home = seeded();
    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    fill_servers(&mut t, &server, smtp.port, &SignIns::default());
    type_password(&mut t, APP_PASSWORD);

    t.wait_until(|s| s.contains(&format!("Signed in as {EMAIL}.")))
        .unwrap();
    assert!(t.wait_exit().unwrap().success());
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains("provider = \"other\""), "{config}");
    let imap = format!(
        "imap = {{ host = \"127.0.0.1\", port = {}, tls = false }}",
        server.port
    );
    assert!(config.contains(&imap), "{config}");
    assert!(config.contains("starttls = false"), "{config}");
    let secret = fs::read_to_string(home.secret_path()).unwrap();
    assert_eq!(secret.trim_end(), APP_PASSWORD);
    let logins: Vec<_> = server
        .commands()
        .into_iter()
        .filter(|c| c.starts_with("LOGIN "))
        .collect();
    assert_eq!(logins.len(), 1, "{logins:?}");
    assert!(logins[0].contains(EMAIL), "{logins:?}");

    // The written config must be one Postal starts, syncs and sends from.
    let mut t = spawn(&home, &[]);
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    send_one(&mut t);
    wait_for(|| !server.appended().is_empty());
    quit(t);
    let plain = format!("\0{EMAIL}\0{APP_PASSWORD}");
    assert_eq!(smtp.logins(), [("PLAIN".to_owned(), plain)]);
    assert_eq!(server.appended()[0].0, "Gesendet");
}

#[test]
fn smtp_signs_in_with_its_own_username_and_password() {
    let (server, smtp) = (minimal(), FakeSmtp::start());
    let home = seeded();
    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    let sign_ins = SignIns {
        imap_user: None,
        smtp: Some(("relay-user", SMTP_PASSWORD)),
    };

    fill_servers(&mut t, &server, smtp.port, &sign_ins);
    t.wait_until(|s| !s.contains(SMTP_PASSWORD)).unwrap();
    type_password(&mut t, APP_PASSWORD);
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let mut t = spawn(&home, &[]);
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    send_one(&mut t);
    wait_for(|| !smtp.received().is_empty());
    quit(t);
    let plain = format!("\0relay-user\0{SMTP_PASSWORD}");
    assert_eq!(smtp.logins(), [("PLAIN".to_owned(), plain)]);
}

// A wrong password must not cost the user the servers typed before it.
#[test]
fn a_refused_password_keeps_the_servers_for_the_next_try() {
    let (server, smtp) = (minimal(), FakeSmtp::start());
    let home = seeded();
    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    fill_servers(&mut t, &server, smtp.port, &SignIns::default());
    type_password(&mut t, "wrong-password");
    t.wait_until(|s| {
        s.contains("Sign-in failed") && s.contains("refused the address or the password")
    })
    .unwrap();
    assert!(!home.secret_path().exists());

    t.send(Key::Enter).unwrap();
    pick_other(&mut t);
    t.wait_until(|s| s.contains(EMAIL)).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(&server.port.to_string()) && s.contains("none"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    type_password(&mut t, APP_PASSWORD);
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

// No IDLE: new mail comes by NOOP. No MOVE: a copy, `\Deleted` and
// `UID EXPUNGE`, to the Folder SPECIAL-USE alone marks Trash.
#[test]
fn a_minimal_server_polls_for_new_mail_and_deletes_to_its_trash() {
    let server = minimal();
    let home = signed_in(&server, 1);
    let mut t = spawn(&home, &[]);
    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    server.deliver(FakeMessage::new("Bob", "Lunch", "2024-11-19T09:00:00Z"));
    // Marking unread asks the sync connection, which ends its wait.
    t.send(Key::Char('U')).unwrap();
    t.wait_until(|s| s.contains("Lunch")).unwrap();

    t.send(Key::Backspace).unwrap();
    // The selection stays on the row it was on as new mail lands above.
    // The copy lands before the expunge: waiting on it alone races the quit.
    wait_for(|| server.subjects("Papierkorb") == ["Weekly notes"]);
    wait_for(|| server.subjects("INBOX") == ["Lunch"]);
    quit(t);
    let commands = server.commands();
    assert!(
        commands.contains(&format!("UID COPY {FIRST_UID} \"Papierkorb\"")),
        "{commands:#?}"
    );
    assert!(
        commands.iter().any(|c| c.starts_with("UID EXPUNGE")),
        "{commands:#?}"
    );
    assert!(commands.iter().any(|c| c == "NOOP"), "{commands:#?}");
    assert!(
        !commands.iter().any(|c| c.contains("X-GM-")),
        "{commands:#?}"
    );
    assert!(
        !commands.iter().any(|c| c.contains("CHANGEDSINCE")),
        "{commands:#?}"
    );
}

// Signing in again starts from the servers written, not from a new guess.
#[test]
fn a_lost_secret_asks_again_at_the_servers_written() {
    let server = minimal();
    let home = signed_in(&server, 1);
    fs::remove_file(home.secret_path()).unwrap();
    let config = fs::read_to_string(home.config_path()).unwrap();
    let config = config.replace("smtp = {", "smtp = { username = \"relay-user\",");
    fs::write(home.config_path(), config).unwrap();
    let smtp_secret = home.secret_path_of(&format!("smtp:{EMAIL}"));
    fs::write(&smtp_secret, format!("{SMTP_PASSWORD}\n")).unwrap();
    fs::set_permissions(&smtp_secret, fs::Permissions::from_mode(0o600)).unwrap();
    let mut t = spawn(&home, &[]);
    t.wait_until(|s| {
        s.contains(&format!("Servers for {EMAIL}"))
            && s.contains(&server.port.to_string())
            && !s.contains("imap.example.com")
    })
    .unwrap();
    t.send(Key::Enter).unwrap();
    type_password(&mut t, APP_PASSWORD);

    t.wait_until(|s| s.contains("Weekly notes")).unwrap();
    quit(t);
    let secret = fs::read_to_string(home.secret_path()).unwrap();
    assert_eq!(secret.trim_end(), APP_PASSWORD);
    // The SMTP password left blank is the one stored, not dropped.
    let kept = fs::read_to_string(&smtp_secret).unwrap();
    assert_eq!(kept.trim_end(), SMTP_PASSWORD);
}

#[test]
fn an_imap_username_signs_in_in_place_of_the_address() {
    let (server, smtp) = (minimal(), FakeSmtp::start());
    server.login_as("imap-user");
    let home = seeded();
    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    let sign_ins = SignIns {
        imap_user: Some("imap-user"),
        smtp: None,
    };

    fill_servers(&mut t, &server, smtp.port, &sign_ins);
    type_password(&mut t, APP_PASSWORD);
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains("username = \"imap-user\""), "{config}");
}

// The top level's servers are every Account's: one at a generic Provider
// leaves it for an `[[account]]` of its own.
#[test]
fn an_account_at_the_top_level_moves_out_for_its_own_servers() {
    let (server, smtp) = (minimal(), FakeSmtp::start());
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\nprovider = \"icloud\"\nsignature = \"Ada\"\n\n[secrets]\nstore = \"file\"\n"
    ));
    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    fill_servers(&mut t, &server, smtp.port, &SignIns::default());
    type_password(&mut t, APP_PASSWORD);
    t.wait_until(|s| s.contains("Sign in at IMAP instead"))
        .unwrap();
    t.send(Key::Char('y')).unwrap();
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let config = fs::read_to_string(home.config_path()).unwrap();
    let (top, account) = config.split_once("[[account]]").expect(&config);
    assert!(!top.contains("email"), "{config}");
    assert!(account.contains(EMAIL), "{config}");
    assert!(account.contains("signature = \"Ada\""), "{config}");
    assert!(account.contains("provider = \"other\""), "{config}");
}

/// Leaves the Onboarding, signed in nowhere.
fn leave(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    t.wait_exit().unwrap();
}

/// An autoconfig and a DNS for the test's domain, publishing nothing yet.
fn published() -> (FakeAutoconfig, FakeDns, Home) {
    let (http, dns) = (FakeAutoconfig::start(), FakeDns::start());
    let home = seeded_at(&http.config(&dns));
    (http, dns, home)
}

#[test]
fn the_domains_own_autoconfig_fills_the_form_and_signs_in() {
    let (server, smtp) = (minimal(), FakeSmtp::start());
    server.login_as("me");
    let (http, _dns, home) = published();
    let own = client_config(&[
        // Postal has no IMAP STARTTLS: the next entry is the one it takes.
        incoming("starttls.example.com", 143, "STARTTLS", "%EMAILADDRESS%"),
        incoming("127.0.0.1", server.port, "plain", "%EMAILLOCALPART%"),
        outgoing("127.0.0.1", smtp.port, "plain", "%EMAILADDRESS%"),
    ]);
    http.publish_own("example.com", &own);
    let ispdb = client_config(&[incoming("imap.ispdb.test", 993, "SSL", "%EMAILADDRESS%")]);
    http.publish_ispdb("example.com", &ispdb);

    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    t.wait_until(|s| {
        s.contains("Published for the domain")
            && s.contains(&server.port.to_string())
            && s.contains(&smtp.port.to_string())
    })
    .unwrap();
    let screen = t.screen().to_string();
    assert!(!screen.contains("starttls.example.com"), "{screen}");
    assert!(!screen.contains("imap.ispdb.test"), "{screen}");
    t.send(Key::Enter).unwrap();
    type_password(&mut t, APP_PASSWORD);
    t.wait_until(|s| s.contains("Signed in")).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let config = fs::read_to_string(home.config_path()).unwrap();
    let imap = format!(
        "imap = {{ host = \"127.0.0.1\", port = {}, tls = false, username = \"me\" }}",
        server.port
    );
    assert!(config.contains(&imap), "{config}");
    // The address goes to no autoconfig: each is asked by the domain alone.
    let paths = http.paths();
    assert!(paths.iter().all(|path| !path.contains('@')), "{paths:?}");
}

#[test]
fn the_ispdb_fills_the_form_when_the_domain_publishes_nothing() {
    let (http, dns, home) = published();
    let ispdb = client_config(&[
        incoming("imap.ispdb.test", 993, "SSL", "%EMAILADDRESS%"),
        outgoing("smtp.ispdb.test", 587, "STARTTLS", "%EMAILADDRESS%"),
    ]);
    http.publish_ispdb("example.com", &ispdb);
    dns.add("_imaps._tcp.example.com", &[(0, 0, 993, "imap.srv.test")]);

    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    t.wait_until(|s| {
        s.contains("imap.ispdb.test") && s.contains("smtp.ispdb.test") && s.contains("STARTTLS")
    })
    .unwrap();
    assert!(!t.screen().to_string().contains("imap.srv.test"));
    leave(t);
}

// RFC 6186: the lowest priority wins, and implicit TLS over STARTTLS.
#[test]
fn srv_records_fill_the_form_without_an_autoconfig() {
    let (_http, dns, home) = published();
    dns.add(
        "_imaps._tcp.example.com",
        &[
            (10, 0, 993, "backup.srv.test"),
            (0, 5, 9993, "imap.srv.test"),
        ],
    );
    dns.add(
        "_submission._tcp.example.com",
        &[(0, 0, 587, "smtp.srv.test")],
    );
    dns.add(
        "_submissions._tcp.example.com",
        &[(0, 0, 4465, "smtps.srv.test")],
    );

    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    t.wait_until(|s| {
        s.contains("imap.srv.test") && s.contains("9993") && s.contains("smtps.srv.test")
    })
    .unwrap();
    assert!(!t.screen().to_string().contains("backup.srv.test"));
    leave(t);
    assert!(
        !dns.asked()
            .iter()
            .any(|name| name.starts_with("_imap._tcp")),
        "{:?}",
        dns.asked()
    );
}

#[test]
fn a_server_published_alone_keeps_the_guess_for_the_other() {
    let (_http, dns, home) = published();
    dns.add("_imaps._tcp.example.com", &[(0, 0, 993, "imap.srv.test")]);
    // `.` says the domain has no such service.
    dns.add("_submissions._tcp.example.com", &[(0, 0, 465, ".")]);

    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    t.wait_until(|s| {
        s.contains("Partly published")
            && s.contains("imap.srv.test")
            && s.contains("smtp.example.com")
    })
    .unwrap();
    leave(t);
}

// A DNS server that never answers must not hold the user at the lookup.
#[test]
fn esc_skips_a_lookup_to_the_guess() {
    let (_http, dns, home) = published();
    for service in ["_imaps", "_submissions", "_submission"] {
        dns.silence(&format!("{service}._tcp.example.com"));
    }

    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    t.wait_until(|s| s.contains(&format!("Looking up the servers for {EMAIL}")))
        .unwrap();
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| s.contains("Guessed from the domain") && s.contains("imap.example.com"))
        .unwrap();
    leave(t);
}

// A DNS server that never answers costs the lookup its timeout, no more.
#[test]
fn a_lookup_that_times_out_falls_back_to_the_guess() {
    let (_http, dns, home) = published();
    dns.silence("_imaps._tcp.example.com");
    dns.add(
        "_submissions._tcp.example.com",
        &[(0, 0, 465, "smtp.srv.test")],
    );

    let mut t = spawn(&home, &["login"]);
    choose_other(&mut t);
    t.wait_until(|s| {
        s.contains("Partly published")
            && s.contains("imap.example.com")
            && s.contains("smtp.srv.test")
    })
    .unwrap();
    leave(t);
}
