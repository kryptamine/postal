//! What `|` hands a recording `$PAGER`: the one Message in the Inbox, as
//! Postal lays it out at 80 columns.

use std::path::Path;

use termlens::{Key, Screen, Terminal};

use super::{
    fake_imap::{FakeImap, FakeMessage},
    home::Home,
};

fn postal(home: &Home, envs: &[(&str, &Path)], no_color: bool) -> Terminal {
    let mut builder = home.terminal();
    for (key, value) in envs {
        builder = builder.env(key, value);
    }

    if no_color {
        builder = builder.env("NO_COLOR", "1");
    }

    builder.spawn(env!("CARGO_BIN_EXE_postal")).unwrap()
}

pub fn main_screen(screen: &Screen) -> bool {
    screen.alternate_screen() && screen.contains(" 1/1 ")
}

// The header and the Snippet show before the body is cached; only text
// under the Preview's rule says `|` will find it.
pub fn previewed(screen: &Screen) -> bool {
    let rule = (0..screen.rows()).find(|&row| screen.row_text(row).starts_with('├'));
    main_screen(screen)
        && rule.is_some_and(|row| {
            !screen
                .row_text(row + 1)
                .trim_matches(|c| c == '│' || c == ' ')
                .is_empty()
        })
}

/// What a recording `$PAGER` read of `message` after `keys`, once the TUI
/// took the screen back.
pub fn paged(message: FakeMessage, keys: &[Key], no_color: bool) -> Vec<u8> {
    let server = FakeImap::start(vec![message]);
    let home = Home::for_server(&server);
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("tmp");
    std::fs::create_dir(&tmp).unwrap();
    let record = dir.path().join("paged");
    // Holds the pager open until the test answers on the terminal, so the
    // suspended screen can be seen.
    let pager = format!(
        "cat > '{}'; printf 'pager done'; read answer < /dev/tty",
        record.display()
    );
    let mut t = postal(
        &home,
        &[("PAGER", Path::new(&pager)), ("TMPDIR", &tmp)],
        no_color,
    );
    t.wait_until(previewed).unwrap();

    for key in keys {
        t.send(*key).unwrap();
    }

    t.wait_until(|s| !s.alternate_screen() && s.contains("pager done"))
        .unwrap();
    t.send(Key::Enter).unwrap();
    t.wait_until(main_screen).unwrap();
    // The TUI still takes keys after the pager.
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let written: Vec<_> = std::fs::read_dir(&tmp).unwrap().collect();
    assert!(written.is_empty(), "{written:?}");
    std::fs::read(&record).unwrap()
}
