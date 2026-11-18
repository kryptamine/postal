//! The Message List shows one row per Thread, grouped by Gmail's
//! `X-GM-THRID`: its senders, count, oldest subject and newest Snippet.

#[allow(dead_code, reason = "each test binary uses part of the harness")]
mod support;

use std::time::{Duration, Instant};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

/// The Message List's columns beside the Preview at 120 columns.
const LIST: usize = 42;

fn postal(home: &Home) -> Terminal {
    Terminal::builder()
        .size(120, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn quit(mut t: Terminal) {
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
}

fn message(sender: &'static str, (subject, body): (&str, &str), date: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(body))
}

/// Thread 0 at UIDs 101, 103 and 104: Alice asks, the Account replies, Bob
/// replies last and Unread. Between them, Carol's lone Message at 102.
fn conversation() -> Vec<FakeMessage> {
    vec![
        message("Alice", ("Trip plans", "Where to?"), "2024-03-01T09:00:00Z").in_thread(0),
        message("Carol", ("Newsletter", "This week"), "2024-03-02T09:00:00Z"),
        message(
            "Me",
            ("Re: Trip plans", "The coast"),
            "2024-03-03T09:00:00Z",
        )
        .in_thread(0)
        .sent_by_me(),
        message(
            "Bob",
            ("Re: Trip plans", "Count me in"),
            "2024-03-04T09:00:00Z",
        )
        .in_thread(0)
        .unread(),
    ]
}

/// Started, with `shown` on screen and the sync idling.
fn ready(server: &FakeImap, home: &Home, shown: &str) -> Terminal {
    let mut t = postal(home);
    t.wait_until(|s| s.contains(shown)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !server.commands().iter().any(|c| c == "IDLE") {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }

    t
}

/// The Message List's rows, top to bottom, trimmed.
fn list(screen: &Screen) -> Vec<String> {
    (0..screen.rows())
        .map(|row| {
            let text: String = screen.row_text(row).chars().take(LIST).collect();
            text.trim_matches(|c: char| c == '│' || c.is_whitespace())
                .to_owned()
        })
        .collect()
}

/// The three lines of the card whose first line starts with `senders`,
/// after any Unread marker.
fn card(screen: &Screen, senders: &str) -> Option<[String; 3]> {
    let rows = list(screen);
    let at = rows.iter().position(|row| {
        row.trim_start_matches('●')
            .trim_start()
            .starts_with(senders)
    })?;
    Some([0, 1, 2].map(|n| rows[at + n].clone()))
}

fn unread(screen: &Screen, senders: &str) -> bool {
    card(screen, senders).is_some_and(|[first, ..]| first.starts_with('●'))
}

/// Whether the list shows `a` above `b`.
fn above(screen: &Screen, a: &str, b: &str) -> bool {
    let rows = list(screen).join("\n");
    rows.find(a).zip(rows.find(b)).is_some_and(|(a, b)| a < b)
}

fn preview(screen: &Screen) -> String {
    (0..screen.rows())
        .map(|row| screen.row_text(row).chars().skip(LIST).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn replies_group_into_one_row_ordered_by_the_newest() {
    let server = FakeImap::start(conversation());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Count me in");
    let screen = t
        .snapshot_after(|s| preview(s).contains("Count me in"))
        .unwrap();
    quit(t);

    let [first, subject, snippet] = card(&screen, "Alice, me, Bob (3)").expect("one card");
    assert!(first.ends_with("2024-03-04"), "{screen}");
    assert_eq!(subject, "Trip plans", "{screen}");
    assert_eq!(snippet, "Count me in", "{screen}");
    assert!(unread(&screen, "Alice, me, Bob"), "{screen}");
    // Bob's reply is the newest arrival, so the Thread sits above Carol.
    assert!(above(&screen, "Alice, me, Bob", "Carol"), "{screen}");
    let rows = list(&screen);
    for gone in ["Where to?", "The coast", "Re: Trip plans"] {
        assert!(!rows.iter().any(|r| r.contains(gone)), "{gone}: {screen}");
    }

    let [carol, ..] = card(&screen, "Carol").expect("Carol's card");
    assert!(!carol.contains('('), "{screen}");
    // The Preview shows the newest Message of the selected Thread.
    assert!(preview(&screen).contains("From    Bob"), "{screen}");
}

#[test]
fn a_thread_is_unread_while_any_of_its_messages_is() {
    let server = FakeImap::start(conversation());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Count me in");
    server.set_seen(104, true);
    let screen = t
        .snapshot_after(|s| s.contains(" Inbox  0 unread "))
        .unwrap();
    assert!(!unread(&screen, "Alice, me, Bob"), "{screen}");

    server.set_seen(101, false);
    let screen = t
        .snapshot_after(|s| s.contains(" Inbox  1 unread "))
        .unwrap();
    quit(t);
    assert!(unread(&screen, "Alice, me, Bob"), "{screen}");
}

#[test]
fn a_reply_arriving_joins_its_thread_and_moves_it_to_the_top() {
    let mut messages = conversation();
    messages.push(message("Dave", ("Lunch", "Noon?"), "2024-03-05T09:00:00Z"));
    let server = FakeImap::start(messages);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Noon?");
    let screen = t
        .snapshot_after(|s| card(s, "Alice, me, Bob (3)").is_some())
        .unwrap();
    assert!(above(&screen, "Dave", "Alice, me, Bob"), "{screen}");

    let reply = message("Erin", ("Re: Trip plans", "Me too"), "2024-03-06T09:00:00Z");
    server.deliver(reply.in_thread(0));
    let screen = t
        .snapshot_after(|s| card(s, "Alice, me, Bob, Erin (4)").is_some())
        .unwrap();
    quit(t);
    assert!(above(&screen, "Alice, me, Bob, Erin", "Dave"), "{screen}");
}

#[test]
fn senders_that_do_not_fit_keep_the_first_and_the_last() {
    let reply = |sender, date| message(sender, ("Re: Big group", "Agreed"), date).in_thread(0);
    let server = FakeImap::start(vec![
        message("Alice", ("Big group", "Hello all"), "2024-03-01T09:00:00Z").in_thread(0),
        reply("Bartholomew", "2024-03-02T09:00:00Z"),
        reply("Cornelius", "2024-03-03T09:00:00Z"),
        reply("Alice", "2024-03-04T09:00:00Z"),
        reply("Dave", "2024-03-05T09:00:00Z"),
    ]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Agreed");
    // Alice wrote twice, but is named once, where she first appears.
    let screen = t
        .snapshot_after(|s| card(s, "Alice … Dave (5)").is_some())
        .unwrap();
    quit(t);
    assert!(
        !list(&screen).iter().any(|r| r.contains("Cornelius")),
        "{screen}"
    );
}

#[test]
fn a_search_finds_a_thread_by_a_reply_sender() {
    let server = FakeImap::start(conversation());
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Count me in");

    t.send(Key::Char('/')).unwrap();
    t.send_str("bob").unwrap();
    let screen = t
        .snapshot_after(|s| card(s, "Carol").is_none() && card(s, "Alice, me, Bob").is_some())
        .unwrap();
    quit(t);

    // The card shows the oldest subject, though only Bob's reply matched.
    let [_, subject, _] = card(&screen, "Alice, me, Bob (3)").expect("one card");
    assert_eq!(subject, "Trip plans", "{screen}");
    // The match is underlined on Bob's name, the third among the senders.
    let row = (0..screen.rows())
        .find(|&row| screen.row_text(row).contains("Alice, me, Bob"))
        .unwrap();
    let text = screen.row_text(row);
    let col = text[..text.find("Bob").unwrap()].chars().count();
    let underlined = |col: usize| {
        screen
            .cell(row, u16::try_from(col).unwrap())
            .unwrap()
            .style()
            .underline
    };
    assert!((col..col + 3).all(underlined), "{screen}");
    assert!(!underlined(col - 4), "{screen}");
}

#[test]
fn a_search_ranks_a_thread_by_its_best_match() {
    let server = FakeImap::start(vec![
        message(
            "Alice",
            ("Invoice", "Please pay by Friday"),
            "2024-03-01T09:00:00Z",
        )
        .in_thread(0),
        message("Bob", ("Re: Invoice", "Done"), "2024-03-02T09:00:00Z").in_thread(0),
        message(
            "Carol",
            ("Report", "Pages and a yearly summary"),
            "2024-03-05T09:00:00Z",
        ),
    ]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Pages and a yearly");
    let screen = t
        .snapshot_after(|s| card(s, "Alice, Bob (2)").is_some())
        .unwrap();
    assert!(above(&screen, "Carol", "Alice, Bob"), "{screen}");

    // `pay` whole in Alice's old Snippet beats Carol's scattered P, a and
    // y, though Carol's Message and Bob's reply match it worse or not at all.
    t.send(Key::Char('/')).unwrap();
    t.send_str("pay").unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("/pay") && above(s, "Alice, Bob", "Carol"))
        .unwrap();
    quit(t);
    assert!(card(&screen, "Carol").is_some(), "{screen}");
}

#[test]
fn a_card_matching_only_part_of_the_query_is_not_highlighted() {
    let server = FakeImap::start(vec![
        message(
            "Alice",
            ("Invoice", "Please pay by Friday"),
            "2024-03-01T09:00:00Z",
        )
        .in_thread(0),
        message("Bob", ("Re: Invoice", "Done"), "2024-03-02T09:00:00Z").in_thread(0),
    ]);
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Done");

    // Alice's Message holds both words, but the card shows only `invoice`:
    // half a match underlined would claim the card answers the Query.
    t.send(Key::Char('/')).unwrap();
    t.send_str("invoice friday").unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("/invoice friday") && card(s, "Alice, Bob").is_some())
        .unwrap();
    quit(t);

    let row = (0..screen.rows())
        .find(|&row| list(&screen)[usize::from(row)] == "Invoice")
        .unwrap();
    let underlined = (0..u16::try_from(LIST).unwrap())
        .any(|col| screen.cell(row, col).unwrap().style().underline);
    assert!(!underlined, "{screen}");
}

#[test]
fn without_gmail_each_message_is_a_row_of_its_own() {
    let server = FakeImap::start(conversation());
    server.without_gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home, "Count me in");
    let screen = t
        .snapshot_after(|s| card(s, "Alice").is_some_and(|[first, ..]| !first.contains(',')))
        .unwrap();
    quit(t);

    for sender in ["Bob", "me", "Carol", "Alice"] {
        let [first, ..] = card(&screen, sender).expect(sender);
        assert!(!first.contains('('), "{sender}: {screen}");
    }

    assert!(above(&screen, "Bob", "me"), "{screen}");
    let commands = server.commands();
    assert!(
        !commands.iter().any(|c| c.contains("X-GM-THRID")),
        "{commands:#?}"
    );
}
