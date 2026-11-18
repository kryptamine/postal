#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::{
    fs,
    time::{Duration, Instant},
};

use base64::Engine;
use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage, Stall},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

fn postal(home: &Home, args: &[&str]) -> Terminal {
    Terminal::builder()
        .size(80, 45)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .args(args)
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Char('q')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The Snippet line of the card whose subject is `subject`; `None` when no
/// card on screen has it.
fn snippet(screen: &Screen, subject: &str) -> Option<String> {
    let row = (0..screen.rows()).find(|&row| screen.row_text(row).contains(subject))?;
    let line = screen.row_text(row + 1);
    Some(
        line.trim_matches(|c: char| c == '│' || c.is_whitespace())
            .to_owned(),
    )
}

fn base64_lines(text: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let lines: Vec<_> = encoded.as_bytes().chunks(76).collect();
    lines
        .iter()
        .map(|line| std::str::from_utf8(line).unwrap())
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn message(subject: &str, day: u32, body: FakeBody) -> FakeMessage {
    FakeMessage::new("Sender", subject, &format!("2024-03-{day:02}T09:00:00Z")).body(body)
}

/// The plain twin is part 1.2, after the HTML one.
fn alternative_with_attachment() -> FakeBody {
    let alternative = FakeBody::multipart(
        "alternative",
        vec![
            FakeBody::text("html", "7bit", "<p>The HTML twin</p>"),
            FakeBody::plain("The plain twin"),
        ],
    );
    FakeBody::multipart(
        "mixed",
        vec![alternative, FakeBody::file("application", "pdf")],
    )
}

/// One Message per kind of body, newest first.
fn kinds() -> Vec<FakeMessage> {
    vec![
        message(
            "Plain",
            7,
            FakeBody::plain("Plain words\r\n\r\non   two lines\r\n"),
        ),
        message(
            "Html only",
            6,
            FakeBody::text(
                "html",
                "7bit",
                "<html><head><title>Hidden title</title><style>p { color: red }</style></head>\r\n\
                 <body><!-- hidden comment --><p>Hello&nbsp;<b>world</b> &amp; friends&#33;</p>\
                 <p>Next&#x2014;line</p></body></html>",
            ),
        ),
        message(
            "Quoted printable",
            5,
            FakeBody::text(
                "plain",
                "quoted-printable",
                "Caf=E9 au lait, a soft line br=\r\neak and =3D sign\r\n",
            )
            .charset("iso-8859-1"),
        ),
        message(
            "Base64",
            4,
            FakeBody::text("plain", "base64", base64_lines("Grüße aus Köln")),
        ),
        message("Alternative", 3, alternative_with_attachment()),
        message(
            "Attachments only",
            2,
            FakeBody::multipart(
                "mixed",
                vec![
                    FakeBody::file("image", "png"),
                    FakeBody::plain("notes.txt, attached").attached(),
                ],
            ),
        ),
    ]
}

const SNIPPETS: [(&str, &str); 5] = [
    ("Plain", "Plain words on two lines"),
    ("Html only", "Hello world & friends! Next—line"),
    (
        "Quoted printable",
        "Café au lait, a soft line break and = sign",
    ),
    ("Base64", "Grüße aus Köln"),
    // A plain part wins over an HTML one, wherever each sits.
    ("Alternative", "The plain twin"),
];

fn all_shown(screen: &Screen) -> bool {
    SNIPPETS
        .iter()
        .all(|(subject, expected)| snippet(screen, subject).as_deref() == Some(expected))
}

#[test]
fn shows_a_snippet_for_each_kind_of_text_part() {
    let server = FakeImap::start(kinds());
    let home = Home::for_server(&server);
    let mut t = postal(&home, &[]);
    let screen = t.snapshot_after(all_shown).unwrap();
    assert_eq!(
        snippet(&screen, "Attachments only").as_deref(),
        Some(""),
        "{screen}"
    );
    // The first card is selected, and the selection is never muted.
    let row = (0..screen.rows())
        .find(|&row| screen.row_text(row).contains("Hello world"))
        .unwrap();
    assert!(screen.cell(row, 3).unwrap().style().dim, "{screen}");
    quit(t);

    // Reading must leave the Message Unread; a Snippet needs only a part's start.
    let commands = server.commands();
    let parts: Vec<_> = commands.iter().filter(|c| c.contains("BODY")).collect();
    assert!(
        parts.iter().any(|c| c.contains("BODY.PEEK[1.2]<0.2048>")),
        "{parts:#?}"
    );
    for command in &parts {
        assert!(!command.contains("BODY["), "{command}");
        assert!(!command.contains("RFC822"), "{command}");
        let peeks = command.matches("BODY.PEEK[").count();
        let partial = command.matches("]<0.2048>").count();
        let header = command.matches("BODY.PEEK[HEADER.FIELDS").count();
        let whole = command.matches("BODY.PEEK[]").count();
        assert_eq!(peeks, partial + header + whole, "{command}");
    }
}

#[test]
fn a_long_part_is_cut_on_a_character_boundary() {
    let server = FakeImap::start(vec![
        message(
            "Long quoted printable",
            2,
            FakeBody::text(
                "plain",
                "quoted-printable",
                "=C3=BC".repeat(400) + "=C3=BC\r\n",
            ),
        ),
        message(
            "Long base64",
            1,
            FakeBody::text("plain", "base64", base64_lines(&"日本語 ".repeat(400))),
        ),
    ]);
    let home = Home::for_server(&server);
    let mut t = postal(&home, &[]);
    wait_for(|| server.commands().iter().any(|c| c == "LOGOUT"));
    t.wait_until(|s| s.contains("üüü")).unwrap();
    quit(t);

    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    let mut statement = cache.prepare("SELECT snippet FROM message").unwrap();
    let snippets: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(snippets.len(), 2);
    for snippet in snippets {
        let length = snippet.chars().count();
        assert!((190..=200).contains(&length), "{length}: {snippet}");
        assert!(snippet.chars().all(|c| "ü日本語 ".contains(c)), "{snippet}");
    }
}

#[test]
fn a_row_never_waits_for_its_snippet() {
    let server = FakeImap::start(kinds());
    server.stall(Stall::Part);
    let home = Home::for_server(&server);
    let mut t = postal(&home, &[]);
    wait_for(|| {
        server
            .commands()
            .iter()
            .any(|c| c.contains("BODY.PEEK[1]<"))
    });
    let waiting = t
        .snapshot_after(|s| s.contains("Attachments only"))
        .unwrap();
    assert_eq!(snippet(&waiting, "Plain").as_deref(), Some(""), "{waiting}");

    server.release();
    t.wait_until(all_shown).unwrap();
    quit(t);
}

#[test]
fn snippets_the_last_run_did_not_reach_come_on_the_next() {
    let server = FakeImap::start(kinds());
    server.stall(Stall::Part);
    let home = Home::for_server(&server);
    let mut t = postal(&home, &[]);
    t.wait_until(|s| s.contains("Attachments only")).unwrap();
    quit(t);

    server.release();
    let mut t = postal(&home, &[]);
    t.wait_until(all_shown).unwrap();
    quit(t);
}

// A short part comes as a quoted string rather than a literal: the wire
// trace must hide it as well.
#[test]
fn snippets_never_reach_the_log() {
    for args in [&[][..], &["-vv"]] {
        let server = FakeImap::start(kinds());
        let home = Home::for_server(&server);
        let mut t = postal(&home, args);
        t.wait_until(all_shown).unwrap();
        wait_for(|| server.commands().iter().any(|c| c == "LOGOUT"));
        quit(t);

        let log = fs::read_to_string(home.log_path()).unwrap();
        assert!(log.contains("Snippets"), "{log}");
        for word in ["Plain words", "friends", "Caf", "twin", "R3L", "notes.txt"] {
            assert!(!log.contains(word), "{word} in {args:?}: {log}");
        }
    }
}
