//! A card of the Message List as the screen shows it, with or without the
//! avatars a list too narrow for them drops.

use termlens::Screen;

/// The column a card's text starts at beside an avatar: the border, the
/// padding, the marks, the four columns of the tile and the gap after it.
const BESIDE_AVATAR: usize = 9;
/// Without one: the border, the padding and the marks.
const PLAIN: usize = 4;

/// Whether the list draws avatars, the top half block of a tile in place.
pub fn avatars(screen: &Screen) -> bool {
    (0..screen.rows()).any(|row| {
        let tile: String = screen.row_text(row).chars().skip(4).take(4).collect();
        tile == "▄▄▄▄"
    })
}

/// The column the text of the cards starts at.
pub fn text_column(screen: &Screen) -> usize {
    if avatars(screen) {
        BESIDE_AVATAR
    } else {
        PLAIN
    }
}

/// Whether the card whose senders are on `row` shows the Unread mark: on
/// that row, or beside an avatar on its middle row.
pub fn unread(screen: &Screen, row: u16) -> bool {
    [row, row + 1].into_iter().any(|row| {
        screen
            .cell(row, 2)
            .is_some_and(|cell| cell.contents() == "●")
    })
}

/// The first row containing `text`.
pub fn row_of(screen: &Screen, text: &str) -> Option<u16> {
    (0..screen.rows()).find(|&row| screen.row_text(row).contains(text))
}

/// The text of the list's `row` right of the marks and avatars, trimmed.
pub fn text(screen: &Screen, row: u16) -> String {
    let line = screen.row_text(row);
    let skip = text_column(screen);
    let text: String = line.chars().skip(skip).take_while(|&c| c != '│').collect();
    text.trim().to_owned()
}

/// The subject on `row`, without the attachment mark and the padding that
/// end its row beside an avatar.
pub fn subject(screen: &Screen, row: u16) -> String {
    let line = screen.row_text(row);
    let text: Vec<char> = line
        .chars()
        .skip(text_column(screen))
        .take_while(|&c| c != '│')
        .collect();
    let mark = if avatars(screen) { 2 } else { 0 };
    let subject: String = text[..text.len().saturating_sub(mark)].iter().collect();
    subject.trim().to_owned()
}

/// The subject line of the card drawn in reverse video, the selection.
pub fn selected_subject(screen: &Screen) -> Option<String> {
    let first =
        (0..screen.rows()).find(|&row| screen.cell(row, 3).is_some_and(|c| c.style().reverse))?;
    Some(subject(screen, first + 1))
}
