//! An avatar starts each card: the sender's initials on a tint its address
//! picks, so a sender is known at a glance.

use crate::support;

use std::fs;

use support::{
    card,
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
    quit,
};
use termlens::{Color, Key, Screen, Terminal};

/// Wide enough for the list to hold an avatar beside the text.
const WIDE: (u16, u16) = (140, 30);

fn postal(home: &Home, (cols, rows): (u16, u16)) -> Terminal {
    home.terminal()
        .size(cols, rows)
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

fn from(name: &'static str, address: &'static str, subject: &str, day: u32) -> FakeMessage {
    let mut message = FakeMessage::new(name, subject, &format!("2024-11-{day:02}T09:00:00Z"));
    message.from_address = address;
    message
}

fn inbox() -> FakeImap {
    FakeImap::start(vec![
        from("Ada Lovelace", "ada@lovelace.org", "Engine notes", 18).unread(),
        from("Bob", "bob@example.com", "Lunch", 17),
        from("", "carol.ng@example.org", "Minutes", 16),
        from("Ada", "Ada@Lovelace.org", "More notes", 15),
    ])
}

/// The tile's three rows on the card whose senders' row holds `sender`.
fn tile(screen: &Screen, sender: &str) -> Option<[String; 3]> {
    let row = card::row_of(screen, sender)?;
    let cols = |row: u16| screen.row_text(row).chars().skip(4).take(4).collect();
    Some([cols(row), cols(row + 1), cols(row + 2)])
}

/// The tint behind the initials on the card of `sender`.
fn tint(screen: &Screen, sender: &str) -> Color {
    let row = card::row_of(screen, sender).unwrap();
    screen.cell(row + 1, 5).unwrap().style().bg
}

fn loaded(screen: &Screen) -> bool {
    screen.contains("More notes")
}

// Two words give their first letters, one word its first two, and no
// name the address's local part; the Unread dot sits level with the letters.
#[test]
fn a_card_starts_with_its_senders_initials() {
    let server = inbox();
    let home = Home::for_server(&server);
    let mut t = postal(&home, WIDE);
    let screen = t.snapshot_after(loaded).unwrap();
    quit(t);

    let shown = |initials: &str| Some(["▄▄▄▄".to_owned(), initials.to_owned(), "▀▀▀▀".to_owned()]);
    assert_eq!(tile(&screen, "Ada Lovelace"), shown(" AL "), "{screen}");
    assert_eq!(tile(&screen, "Bob"), shown(" BO "), "{screen}");
    assert_eq!(tile(&screen, "carol.ng"), shown(" CN "), "{screen}");

    let ada = card::row_of(&screen, "Ada Lovelace").unwrap();
    assert_eq!(screen.cell(ada + 1, 2).unwrap().contents(), "●", "{screen}");
    assert!(
        screen.cell(ada, 2).unwrap().contents().trim().is_empty(),
        "{screen}"
    );
}

// The hue hashes the address without case, never a per-run seed: a sender
// is the same colour on each of their cards and in the next run.
#[test]
fn a_sender_keeps_one_colour_on_every_card_and_every_run() {
    let server = inbox();
    let home = Home::for_server(&server);
    let mut t = postal(&home, WIDE);
    let first = t.snapshot_after(loaded).unwrap();
    quit(t);
    let mut t = postal(&home, WIDE);
    let second = t.snapshot_after(loaded).unwrap();
    quit(t);

    let ada = tint(&first, "Ada Lovelace");
    assert!(matches!(ada, Color::Rgb(..)), "{first}");
    assert_eq!(tint(&first, "Ada   "), ada, "{first}");
    assert_eq!(tint(&second, "Ada Lovelace"), ada, "{second}");
    assert_ne!(tint(&first, "Bob"), ada, "{first}");
}

// Beside the Preview in a terminal 100 columns wide the list has no room
// for an avatar, and the card keeps its marks left of the text.
#[test]
fn a_list_too_narrow_keeps_the_cards_as_text() {
    let server = inbox();
    let home = Home::for_server(&server);
    let mut t = postal(&home, (100, 30));
    let screen = t.snapshot_after(loaded).unwrap();
    quit(t);

    assert!(!card::avatars(&screen), "{screen}");
    let ada = card::row_of(&screen, "Ada Lovelace").unwrap();
    assert_eq!(screen.cell(ada, 2).unwrap().contents(), "●", "{screen}");
}

// The switch on the Icons tab writes `avatars` to `config.toml`, and the
// running list and the next run follow.
#[test]
fn settings_hides_the_avatars_at_once_and_in_the_next_run() {
    let server = inbox();
    let home = Home::for_server(&server);
    let mut t = postal(&home, WIDE);
    t.wait_until(|s| loaded(s) && card::avatars(s)).unwrap();
    t.send(Key::Char(',')).unwrap();
    t.wait_until(|s| s.contains("Settings")).unwrap();
    t.send(Key::Tab).unwrap();
    t.wait_until(|s| s.contains("an avatar beside each card"))
        .unwrap();
    for _ in 0..3 {
        t.send(Key::Char('j')).unwrap();
    }

    t.wait_until(|s| s.contains("↵ hide")).unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(|s| s.contains("↵ show")).unwrap();
    let config = fs::read_to_string(home.config_path()).unwrap();
    assert!(config.contains("avatars = \"hide\""), "{config}");
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| !s.contains("Settings") && !card::avatars(s))
        .unwrap();
    quit(t);

    let mut t = postal(&home, WIDE);
    let screen = t.snapshot_after(loaded).unwrap();
    quit(t);
    assert!(!card::avatars(&screen), "{screen}");
}
