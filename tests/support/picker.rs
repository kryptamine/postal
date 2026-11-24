use termlens::{Key, Screen, Terminal};

/// The popup's rows inside its border, trimmed: the columns between the
/// corners of the box titled `Folders`. Empty until the whole box is drawn,
/// since a snapshot can land between the bytes of one frame.
pub fn picker(screen: &Screen) -> Vec<String> {
    titled(screen, "Folders")
}

/// As `picker`, while it picks the Folder to move Messages to.
pub fn mover(screen: &Screen) -> Vec<String> {
    titled(screen, "Move to")
}

fn titled(screen: &Screen, title: &str) -> Vec<String> {
    let title = format!("┌ {title}");
    let chars = |row: u16| screen.row_text(row).chars().collect::<Vec<_>>();
    let Some(top) = (0..screen.rows()).find(|&r| screen.row_text(r).contains(&title)) else {
        return Vec::new();
    };

    let corners = chars(top);
    // The Preview's title row starts with a corner of its own.
    let title: Vec<char> = title.chars().collect();
    let left = corners
        .windows(title.len())
        .position(|w| w == title)
        .unwrap();
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

/// The picker's Folder rows, each its label and its Unread count if shown,
/// the scrollbar left out; none while it says `no matches`, as before the
/// Folders are listed. The first two rows are the Query and the rule.
pub fn picked(screen: &Screen) -> Vec<String> {
    rows(picker(screen))
}

/// As `picked`, while the picker picks the Folder to move Messages to.
pub fn move_rows(screen: &Screen) -> Vec<String> {
    rows(mover(screen))
}

fn rows(popup: Vec<String>) -> Vec<String> {
    popup
        .into_iter()
        .skip(2)
        .filter(|row| !row.is_empty() && row != "no matches")
        .map(|row| {
            let row = row.trim_end_matches('▐');
            row.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .collect()
}

/// Opens the Folder Picker until its row for a Folder reads `row`, as
/// `Inbox 2`, then closes it.
pub fn wait_picked(t: &mut Terminal, row: &str) {
    t.send(Key::Char('f')).unwrap();
    t.wait_until(|s| picked(s).iter().any(|r| r == row))
        .unwrap();
    t.send(Key::Esc).unwrap();
    t.wait_until(|s| picker(s).is_empty()).unwrap();
}
