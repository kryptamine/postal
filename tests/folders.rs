//! Folders: `f` opens the Folder Picker over the screen, and `Enter` shows
//! the chosen Folder in the Message List.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::time::{Duration, Instant};

use support::{
    fake_body::FakeBody,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};
use termlens::{Key, Screen, Terminal};

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

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn message(sender: &'static str, subject: &str, date: &str) -> FakeMessage {
    FakeMessage::new(sender, subject, date).body(FakeBody::plain(&format!("Body of {subject}")))
}

/// A Gmail account: the Inbox, the `[Gmail]` parent it cannot select, its
/// special-use Folders listed out of order, and labels.
fn gmail() -> FakeImap {
    let server = FakeImap::start(vec![
        message("Alice", "Inbox older", "2024-03-01T09:00:00Z"),
        message("Bob", "Inbox newest", "2024-03-02T09:00:00Z").unread(),
    ]);
    server.add_folder(
        "Work",
        &[],
        vec![
            message("Carol", "Work older", "2024-02-01T09:00:00Z").unread(),
            message("Dave", "Work newest", "2024-02-02T09:00:00Z").unread(),
        ],
    );
    server.add_folder("[Gmail]", &["\\Noselect"], vec![]);
    server.add_folder(
        "[Gmail]/Trash",
        &["\\Trash"],
        vec![message("Eve", "Thrown away", "2024-01-01T09:00:00Z")],
    );
    server.add_folder(
        "[Gmail]/Sent Mail",
        &["\\Sent"],
        vec![
            message("Me", "Sent older", "2024-01-02T09:00:00Z"),
            message("Me", "Sent newest", "2024-01-03T09:00:00Z"),
        ],
    );
    server.add_folder("Archive", &[], vec![]);
    server.add_folder("[Gmail]/Starred", &["\\Flagged"], vec![]);
    server.add_folder(
        "Work/Clients",
        &[],
        vec![message("Frank", "Client brief", "2024-02-03T09:00:00Z").unread()],
    );
    server.add_folder("[Gmail]/Spam", &["\\Junk"], vec![]);
    server.add_folder("[Gmail]/All Mail", &["\\All"], vec![]);
    server.add_folder("[Gmail]/Drafts", &["\\Drafts"], vec![]);
    server
}

fn idles(server: &FakeImap) -> usize {
    server.commands().iter().filter(|c| *c == "IDLE").count()
}

/// Started, with the Inbox shown and the sync idling.
fn ready(server: &FakeImap, home: &Home) -> Terminal {
    let mut t = postal(home);
    t.wait_until(|s| s.contains("Inbox newest")).unwrap();
    wait_for(|| idles(server) > 0);
    t
}

/// The popup's rows inside its border, trimmed: the columns between the
/// corners of the box titled `Folders`. Empty until the whole box is drawn,
/// since a snapshot can land between the bytes of one frame.
fn picker(screen: &Screen) -> Vec<String> {
    let chars = |row: u16| screen.row_text(row).chars().collect::<Vec<_>>();
    let Some(top) = (0..screen.rows()).find(|&r| screen.row_text(r).contains("┌ Folders")) else {
        return Vec::new();
    };

    let corners = chars(top);
    let left = corners.iter().position(|&c| c == '┌').unwrap();
    let Some(right) = corners[left..].iter().position(|&c| c == '┐') else {
        return Vec::new();
    };

    let right = left + right;
    let Some(bottom) = (top + 1..screen.rows()).find(|&r| chars(r).get(left) == Some(&'└'))
    else {
        return Vec::new();
    };

    (top + 1..bottom)
        .map(chars)
        .map(|row| {
            row[left + 1..right]
                .iter()
                .collect::<String>()
                .trim()
                .to_owned()
        })
        .collect()
}

/// The picker's Folder rows, each its label and its Unread count if shown.
/// The first two rows are the Query and the rule under it.
fn picked(screen: &Screen) -> Vec<String> {
    picker(screen)
        .into_iter()
        .skip(2)
        .filter(|row| !row.is_empty())
        .map(|row| row.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

fn title(screen: &Screen) -> String {
    screen.row_text(0)
}

#[test]
fn the_picker_lists_special_use_folders_first_then_the_rest_with_unread_counts() {
    let server = gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    server.clear_commands();

    t.send(Key::Char('f')).unwrap();
    let screen = t
        .snapshot_after(|s| picked(s).iter().any(|row| row == "Work 2"))
        .unwrap();
    assert_eq!(
        picked(&screen),
        [
            "Inbox 1",
            "Starred",
            "Sent",
            "Drafts",
            "All Mail",
            "Spam",
            "Trash",
            "Archive",
            "Work 2",
            "Work/Clients 1",
        ],
        "{screen}"
    );
    // The Query stands apart from the Folders.
    assert!(picker(&screen)[1].chars().all(|c| c == '─'), "{screen}");

    // IDLE gives way to the listing, and comes back after it. The shown
    // Folder's count is its own, never a `STATUS`.
    wait_for(|| idles(&server) > 0);
    let commands = server.commands();
    let at = |command: &str| commands.iter().position(|c| c.starts_with(command));
    let list = at("LIST").unwrap();
    assert!(at("DONE").unwrap() < list, "{commands:?}");
    assert!(commands[list..].iter().any(|c| c == "IDLE"), "{commands:?}");
    let statuses: Vec<_> = commands
        .iter()
        .filter(|c| c.starts_with("STATUS"))
        .collect();
    assert_eq!(statuses.len(), 9, "{commands:?}");
    assert!(
        statuses.iter().all(|c| !c.contains("INBOX")),
        "{commands:?}"
    );
    assert!(
        !statuses.iter().any(|c| c.contains("\"[Gmail]\"")),
        "{commands:?}"
    );
    assert!(title(&screen).contains("Inbox  1 unread"), "{screen}");
    quit(t);
}

/// The subject line of the card drawn in reverse video, the selection.
fn selected_subject(screen: &Screen) -> Option<String> {
    let first =
        (0..screen.rows()).find(|&row| screen.cell(row, 3).is_some_and(|c| c.style().reverse))?;
    let subject = screen.row_text(first + 1);
    let subject = subject.split('│').nth(1)?;
    Some(subject.trim().to_owned())
}

/// Opens the picker, types `query` and shows the best match.
fn show(t: &mut Terminal, query: &str) {
    t.send(Key::Char('f')).unwrap();
    t.wait_until(|s| !picker(s).is_empty()).unwrap();
    t.send_str(query).unwrap();
    // A first start lists the Cache's Inbox alone until the server's list
    // comes, and Enter on no match does nothing. Settled, since mid-frame the
    // old rows can still stand under the new query.
    t.snapshot_after(|s| {
        picker(s).first().is_some_and(|q| q.ends_with(query)) && !picked(s).is_empty()
    })
    .unwrap();
    t.send(Key::Enter).unwrap();
}

#[test]
fn a_query_narrows_the_picker_and_enter_shows_the_folder_live() {
    let server = gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('f')).unwrap();
    t.send_str("wrk").unwrap();
    let screen = t
        .snapshot_after(|s| picked(s) == ["Work 2", "Work/Clients 1"])
        .unwrap();
    assert!(picker(&screen)[0].ends_with("wrk"), "{screen}");
    // `^n` and the arrows move; the selection is drawn in reverse video.
    t.send(Key::Ctrl('n')).unwrap();
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| title(s).contains("Work/Clients  1 unread"))
        .unwrap();
    assert!(picker(&screen).is_empty(), "{screen}");
    let screen = t.snapshot_after(|s| s.contains("Client brief")).unwrap();
    assert!(!screen.contains("Inbox newest"), "{screen}");
    assert_eq!(selected_subject(&screen).as_deref(), Some("Client brief"));
    t.wait_until(|s| s.contains("Body of Client brief"))
        .unwrap();

    // IDLE now watches the Folder shown.
    wait_for(|| {
        let commands = server.commands();
        let examine = commands
            .iter()
            .position(|c| c == "EXAMINE \"Work/Clients\"");
        examine.is_some_and(|at| commands[at..].iter().any(|c| c == "IDLE"))
    });
    server.deliver_to(
        "Work/Clients",
        message("Grace", "Client reply", "2024-02-04T09:00:00Z").unread(),
    );
    let screen = t
        .snapshot_after(|s| s.contains("Client reply") && title(s).contains("2 unread"))
        .unwrap();
    assert!(screen.contains("Client brief"), "{screen}");

    // Back to the Inbox by its label.
    show(&mut t, "inbox");
    let screen = t.snapshot_after(|s| s.contains("Inbox newest")).unwrap();
    assert!(title(&screen).contains("Inbox  1 unread"), "{screen}");
    assert!(!screen.contains("Client brief"), "{screen}");
    quit(t);
}

#[test]
fn from_reading_escape_comes_back_to_the_message_and_enter_goes_to_the_list() {
    let server = gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    t.send(Key::Char('j')).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("Body of Inbox older") && !s.contains("Inbox newest"))
        .unwrap();

    t.send(Key::Char('f')).unwrap();
    t.wait_until(|s| !picker(s).is_empty()).unwrap();
    t.send(Key::Esc).unwrap();
    let screen = t.snapshot_after(|s| picker(s).is_empty()).unwrap();
    assert!(screen.contains("Body of Inbox older"), "{screen}");
    assert!(!screen.contains("Inbox newest"), "{screen}");

    show(&mut t, "sent");
    let screen = t
        .snapshot_after(|s| title(s).contains("Sent  0 unread") && s.contains("Sent older"))
        .unwrap();
    assert_eq!(selected_subject(&screen).as_deref(), Some("Sent newest"));
    quit(t);
}

#[test]
fn switching_ends_a_search_and_selects_the_newest_message() {
    let server = gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    t.send(Key::Char('/')).unwrap();
    t.send_str("older").unwrap();
    t.send(Key::Enter).unwrap();
    t.snapshot_after(|s| selected_subject(s).as_deref() == Some("Inbox older"))
        .unwrap();

    show(&mut t, "work");
    let screen = t
        .snapshot_after(|s| selected_subject(s).as_deref() == Some("Work newest"))
        .unwrap();
    assert!(screen.contains("Work older"), "{screen}");
    assert!(
        !screen.row_text(screen.rows() - 1).contains("/older"),
        "{screen}"
    );

    // A Folder seen before comes back on its newest Message too.
    t.send(Key::Char('j')).unwrap();
    t.snapshot_after(|s| selected_subject(s).as_deref() == Some("Work older"))
        .unwrap();
    show(&mut t, "inbox");
    t.snapshot_after(|s| selected_subject(s).as_deref() == Some("Inbox newest"))
        .unwrap();
    show(&mut t, "work");
    t.snapshot_after(|s| selected_subject(s).as_deref() == Some("Work newest"))
        .unwrap();
    quit(t);
}

fn cached_rows(home: &Home, folder: &str) -> i64 {
    let cache = rusqlite::Connection::open(home.cache_path()).unwrap();
    cache
        .query_row(
            "SELECT count(*) FROM message WHERE mailbox = ?1",
            [folder],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn a_folder_gone_from_the_server_leaves_the_cache_and_the_inbox_shows() {
    let server = gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    show(&mut t, "work/clients");
    t.wait_until(|s| s.contains("Client brief")).unwrap();
    wait_for(|| cached_rows(&home, "Work/Clients") == 1);

    server.remove_folder("Work/Clients");
    t.send(Key::Char('f')).unwrap();
    let screen = t
        .snapshot_after(|s| s.contains("Folder Work/Clients no longer exists"))
        .unwrap();
    let screen = if screen.contains("Inbox newest") {
        screen
    } else {
        t.snapshot_after(|s| s.contains("Inbox newest")).unwrap()
    };
    assert!(title(&screen).contains("Inbox"), "{screen}");
    t.wait_until(|s| picked(s).iter().any(|row| row == "Work 2"))
        .unwrap();
    let screen = t.snapshot_after(|s| !picked(s).is_empty()).unwrap();
    assert!(
        !picked(&screen)
            .iter()
            .any(|row| row.starts_with("Work/Clients")),
        "{screen}"
    );
    assert_eq!(cached_rows(&home, "Work/Clients"), 0);
    quit(t);
}

#[test]
fn offline_the_picker_and_a_cached_folder_come_from_the_cache() {
    let server = gmail();
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);
    show(&mut t, "work");
    t.wait_until(|s| s.contains("Work older")).unwrap();
    wait_for(|| cached_rows(&home, "Work") == 2);
    t.send(Key::Char('f')).unwrap();
    t.wait_until(|s| picked(s).iter().any(|row| row == "Work/Clients 1"))
        .unwrap();
    t.send(Key::Esc).unwrap();
    quit(t);

    server.fail(support::fake_imap::Outage::Offline);
    let mut t = postal(&home);
    // Every run starts in the Inbox.
    let screen = t
        .snapshot_after(|s| s.contains("Inbox newest") && s.contains("offline"))
        .unwrap();
    assert!(title(&screen).contains("Inbox  1 unread"), "{screen}");

    t.send(Key::Char('f')).unwrap();
    let screen = t
        .snapshot_after(|s| picked(s).iter().any(|row| row == "Work/Clients 1"))
        .unwrap();
    assert_eq!(picked(&screen).len(), 10, "{screen}");
    t.send_str("work").unwrap();
    t.wait_until(|s| picked(s).first().is_some_and(|f| f == "Work 2"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| title(s).contains("Work  2 unread") && s.contains("Work older"))
        .unwrap();
    assert!(screen.contains("offline"), "{screen}");

    // A Folder never shown has no rows to show until connected.
    show(&mut t, "trash");
    let screen = t.snapshot_after(|s| title(s).contains("Trash")).unwrap();
    assert!(!screen.contains("Work older"), "{screen}");
    assert!(!screen.contains("Thrown away"), "{screen}");
    quit(t);
}

#[test]
fn a_folder_named_in_modified_utf7_shows_decoded_and_opens_by_its_server_name() {
    let server = gmail();
    server.add_folder(
        "&BBAEQARFBDgEMg-",
        &[],
        vec![message("Ivan", "Old letter", "2023-05-01T09:00:00Z")],
    );
    let home = Home::for_server(&server);
    let mut t = ready(&server, &home);

    t.send(Key::Char('f')).unwrap();
    t.send_str("архив").unwrap();
    t.wait_until(|s| picked(s).first().is_some_and(|f| f == "Архив"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    let screen = t
        .snapshot_after(|s| title(s).contains("Архив") && s.contains("Old letter"))
        .unwrap();
    assert!(!screen.contains("&BBAE"), "{screen}");
    assert!(
        server
            .commands()
            .iter()
            .any(|c| c == "EXAMINE \"&BBAEQARFBDgEMg-\""),
        "{:?}",
        server.commands()
    );
    quit(t);
}
