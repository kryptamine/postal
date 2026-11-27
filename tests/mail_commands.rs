//! The commands that read the mail a TUI synced, for a person or an agent,
//! name each Thread and Message by an ID another command takes; they only
//! read the Cache. See ADR 0017 and ADR 0030.

use crate::support;

use std::process::Output;

use serde_json::Value;
use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
    picker::{picked, picker},
};
use termlens::{Key, Terminal};

fn from_ada(subject: &str, date: &str, body: &str) -> FakeMessage {
    FakeMessage {
        from_address: "ada@example.com",
        ..FakeMessage::new("Ada Lovelace", subject, date)
    }
    .body(FakeBody::plain(body))
    .in_thread(0)
}

/// Newest first: Ada's Thread of two, the newer Unread, then Bob's invoice.
fn inbox() -> Vec<FakeMessage> {
    vec![
        from_ada("Weekly notes", "2024-11-17T09:00:00Z", "First notes"),
        from_ada("Re: Weekly notes", "2024-11-18T09:00:00Z", "Second notes").unread(),
        FakeMessage {
            from_address: "bob@example.com",
            ..FakeMessage::new("Bob", "Invoice 1042", "2024-11-16T09:00:00Z")
        }
        .body(FakeBody::plain("Please pay")),
    ]
}

/// The TUI until the newest body is cached, Sent and Trash opened once so
/// the Cache holds them too.
fn synced(server: &FakeImap, home: &Home) {
    server.add_folder(
        "[Gmail]/Sent Mail",
        &["\\Sent"],
        vec![FakeMessage::new("Me", "Rent", "2024-11-10T09:00:00Z").sent_by_me()],
    );
    server.add_folder(
        "[Gmail]/Trash",
        &["\\Trash"],
        vec![FakeMessage::new("Eve", "Old offer", "2024-11-09T09:00:00Z").in_thread(1)],
    );
    let mut t = home.terminal().spawn(env!("CARGO_BIN_EXE_postal")).unwrap();
    t.wait_until(|s| s.contains("Second notes")).unwrap();
    open(&mut t, "sent", "Rent");
    open(&mut t, "trash", "Old offer");
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn open(t: &mut Terminal, query: &str, row: &str) {
    t.send(Key::Char('f')).unwrap();
    t.wait_until(|s| !picker(s).is_empty()).unwrap();
    t.send_str(query).unwrap();
    t.wait_until(|s| {
        picker(s).first().is_some_and(|q| q.ends_with(query)) && !picked(s).is_empty()
    })
    .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains(row)).unwrap();
}

fn postal(home: &Home, args: &[&str]) -> Output {
    home.command(env!("CARGO_BIN_EXE_postal"))
        .args(args)
        .output()
        .unwrap()
}

/// The `--json` answer to `args`.
fn json(home: &Home, args: &[&str]) -> Vec<Value> {
    let mut args = args.to_vec();
    args.push("--json");
    let output = postal(home, &args);
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn text(rows: &[Value], field: &str) -> Vec<String> {
    rows.iter()
        .map(|row| row[field].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn a_listing_names_each_thread_and_message_for_the_commands_that_show_them() {
    let server = FakeImap::start(inbox());
    let home = Home::for_server(&server);
    synced(&server, &home);

    let listed = json(&home, &["message", "list"]);
    assert_eq!(text(&listed, "subject"), ["Weekly notes", "Invoice 1042"]);
    let ada = &listed[0];
    assert_eq!(ada["messages"], 2);
    assert_eq!(ada["unread"], true);
    assert_eq!(ada["snippet"], "Second notes");
    assert_eq!(ada["senders"][0], "Ada Lovelace <ada@example.com>");
    assert_eq!(ada["date"], "2024-11-18T09:00:00+00:00");

    let thread = ada["thread"].as_str().unwrap();
    let shown = json(&home, &["thread", "show", thread]);
    assert_eq!(
        text(&shown, "subject"),
        ["Weekly notes", "Re: Weekly notes"]
    );
    // Only the Preview fetched a body: the older is marked, not skipped.
    assert!(shown[0]["body"].is_null());
    assert_eq!(shown[1]["body"], "Second notes");
    assert_eq!(shown[1]["message"], ada["message"]);

    let message = json(
        &home,
        &["message", "show", ada["message"].as_str().unwrap()],
    );
    assert_eq!(message[0]["subject"], "Re: Weekly notes");
    assert_eq!(message[0]["body"], "Second notes");
    assert_eq!(message[0]["thread"], thread);
    assert_eq!(message[0]["folder"], "INBOX");

    // The text a person reads names the Thread too.
    let output = postal(&home, &["message", "list"]);
    let table = String::from_utf8(output.stdout).unwrap();
    assert!(table.lines().next().unwrap().starts_with("* "), "{table}");
    assert!(table.contains(thread), "{table}");
}

#[test]
fn a_folder_is_named_by_its_role_or_its_name() {
    let server = FakeImap::start(inbox());
    // «Архив» in modified UTF-7, as the server lists it.
    server.add_folder("&BBAEQARFBDgEMg-", &[], vec![]);
    let home = Home::for_server(&server);
    synced(&server, &home);

    let folders = json(&home, &["folder", "list"]);
    let sent = folders
        .iter()
        .find(|f| f["name"] == "[Gmail]/Sent Mail")
        .unwrap();
    assert_eq!(sent["role"], "Sent");
    // A name is shown, and taken, as people read it.
    assert!(folders.iter().any(|f| f["name"] == "Архив"), "{folders:?}");
    // A total is counted beside the Unread, and shown when nothing is Unread.
    assert_eq!(sent["total"], 1);
    assert_eq!(sent["unread"], 0);
    let inbox = folders.iter().find(|f| f["name"] == "INBOX").unwrap();
    assert_eq!((&inbox["total"], &inbox["unread"]), (&3.into(), &1.into()));
    let table = String::from_utf8(postal(&home, &["folder", "list"]).stdout).unwrap();
    assert!(!table.contains('&'), "{table}");
    let line = |name: &str| table.lines().find(|l| l.starts_with(name)).unwrap();
    assert!(line("INBOX").ends_with("  3 messages, 1 unread"), "{table}");
    assert!(
        line("[Gmail]/Sent Mail").ends_with("  1 message"),
        "{table}"
    );
    for folder in ["архив", "&BBAEQARFBDgEMg-"] {
        assert!(
            postal(&home, &["message", "list", "--folder", folder])
                .status
                .success()
        );
    }

    for folder in ["Sent", "sent", "[Gmail]/Sent Mail"] {
        let listed = json(&home, &["message", "list", "--folder", folder]);
        assert_eq!(text(&listed, "subject"), ["Rent"]);
    }

    // A Thread wholly in Trash still shows by the ID its listing gave.
    let trash = json(&home, &["message", "list", "--folder", "Trash"]);
    let thread = trash[0]["thread"].as_str().unwrap();
    let shown = json(&home, &["thread", "show", thread]);
    assert_eq!(text(&shown, "subject"), ["Old offer"]);

    let output = postal(&home, &["message", "list", "--folder", "Receipts"]);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "postal: no Account has a Folder Receipts\n"
    );
}

#[test]
fn a_search_finds_threads_by_the_start_of_a_word() {
    let server = FakeImap::start(inbox());
    let home = Home::for_server(&server);
    synced(&server, &home);

    let found = json(&home, &["message", "search", "invo"]);
    assert_eq!(text(&found, "subject"), ["Invoice 1042"]);
    // Both Messages of a Thread match, and it is one row.
    let found = json(&home, &["message", "search", "weekly"]);
    assert_eq!(text(&found, "subject"), ["Weekly notes"]);
    assert_eq!(found[0]["messages"], 2);
    // A word inside another is no match.
    assert_eq!(
        json(&home, &["message", "search", "voice"]),
        Vec::<Value>::new()
    );
    assert_eq!(json(&home, &["message", "list", "--limit", "1"]).len(), 1);
}

#[test]
fn a_search_takes_the_operators_the_tui_does() {
    let server = FakeImap::start(inbox());
    let home = Home::for_server(&server);
    synced(&server, &home);

    for (query, subjects) in [
        ("from:ada is:unread", &["Re: Weekly notes"][..]),
        ("from:bob@example.com", &["Invoice 1042"]),
        ("-from:ada before:2024-11-17", &["Invoice 1042", "Rent"]),
        ("in:trash", &["Old offer"]),
        ("notes -is:unread", &["Weekly notes"]),
    ] {
        let found = json(&home, &["message", "search", query]);
        assert_eq!(text(&found, "subject"), subjects, "{query}");
    }

    let output = postal(&home, &["message", "search", "from:"]);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "postal: the Query has no word or operator to search by\n"
    );
}

#[test]
fn an_id_postal_never_handed_out_is_refused() {
    let server = FakeImap::start(inbox());
    let home = Home::for_server(&server);
    synced(&server, &home);

    for (args, error) in [
        (["message", "show", "nonsense"], "nonsense is no Message ID"),
        (["thread", "show", "m0000"], "m0000 is no Thread ID"),
        (
            ["message", "show", "m0000000000000000z"],
            "m0000000000000000z names no Account set up",
        ),
    ] {
        let output = postal(&home, &args);
        assert!(!output.status.success());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!("postal: {error}\n")
        );
    }
}

#[test]
fn a_cache_of_another_schema_is_refused_as_none() {
    let server = FakeImap::start(inbox());
    let home = Home::for_server(&server);
    synced(&server, &home);
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    cache.pragma_update(None, "user_version", 9999).unwrap();
    drop(cache);

    let output = postal(&home, &["message", "list"]);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "postal: Cache not ready: run `postal` once\n"
    );
}

#[test]
fn without_a_cache_each_command_says_so() {
    let server = FakeImap::start(inbox());
    let home = Home::for_server(&server);
    for args in [
        &["folder", "list"][..],
        &["message", "list"],
        &["message", "search", "notes"],
    ] {
        let output = postal(&home, args);
        assert!(!output.status.success());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "postal: Cache not ready: run `postal` once\n"
        );
    }
}

// Some senders put HTML into the plain part: what a command shows, and so
// a reply quotes, is its text, from the HTML twin when there is one.
#[test]
fn a_plain_part_carrying_html_is_shown_as_text() {
    let document = "<!DOCTYPE html><html><body><p>Hi&nbsp;there</p></body></html>\r\n";
    let cases = [
        (
            FakeBody::multipart(
                "alternative",
                vec![
                    FakeBody::plain(&format!("Like summer.\r\n\r\n{document}")),
                    FakeBody::text("html", "7bit", "<p>Summer is <b>over</b></p>"),
                ],
            ),
            "Summer is over",
        ),
        (
            FakeBody::plain(&format!("Tom & Jerry, 2 < 3.\r\n\r\n{document}")),
            "Tom & Jerry, 2 < 3.\n\nHi there",
        ),
    ];
    for (body, expected) in cases {
        let server = FakeImap::start(vec![
            from_ada("Markup", "2024-11-17T09:00:00Z", "").body(body),
        ]);
        let home = Home::for_server(&server);
        let mut t = home.terminal().spawn(env!("CARGO_BIN_EXE_postal")).unwrap();
        // Only the Preview shows either, once the body is cached.
        t.wait_until(|s| s.contains("Summer is over") || s.contains("<!DOCTYPE"))
            .unwrap();
        t.send(Key::Ctrl('c')).unwrap();
        assert!(t.wait_exit().unwrap().success());

        let listed = json(&home, &["message", "list"]);
        let id = listed[0]["message"].as_str().unwrap();
        let shown = json(&home, &["message", "show", id]);
        assert_eq!(shown[0]["body"], expected);
    }
}
