use ratatui::layout::{Position, Rect};

use crate::{ui::frame::layout::PADDING, ui::list::avatar};

/// The marks' column and its blank, since a Nerd Font glyph spills over it.
const MARKS: u16 = 2;
/// The marks, the tile and the gap after it.
const BESIDE_AVATAR: u16 = MARKS + avatar::WIDTH + 1;
/// Below this card width there is no avatar, so the text keeps 25 columns.
const AVATAR_MIN_WIDTH: u16 = 34;
/// The date, a `DateLabel` of at most 10 columns, and the gap before it.
const DATE_ROOM: u16 = 11;

/// Where each part of one card goes. Beside an avatar the status mark sits
/// level with its letters and the attachment mark ends the subject's row;
/// without one both stand left of the text.
pub struct CardLayout {
    pub avatar: Option<Rect>,
    /// The Unread or answered mark.
    pub status: Option<Position>,
    pub attachment: Option<Position>,
    pub senders: Rect,
    pub subject: Rect,
    pub snippet: Rect,
    /// The column the date ends before, on the senders' row.
    pub right: u16,
}

impl CardLayout {
    /// `avatars` when they are wanted, `dated` when the date has room, and
    /// `attached` when the card has the attachment mark. A part past the
    /// bottom of `card` is empty or `None`.
    pub fn of(card: Rect, avatars: bool, dated: bool, attached: bool) -> Self {
        // The selection fills the whole card; only its text is padded.
        let (x, y, right) = (
            card.x + PADDING,
            card.y,
            card.right().saturating_sub(PADDING),
        );
        let avatar = (avatars && card.width >= AVATAR_MIN_WIDTH)
            .then(|| Rect::new(x + MARKS, y, avatar::WIDTH, card.height.min(3)));
        let (indent, status, attachment, clip) = match avatar {
            Some(_) => {
                let clip = if attached { MARKS } else { 0 };
                (
                    BESIDE_AVATAR,
                    (x, y + 1),
                    (right.saturating_sub(1), y + 1),
                    clip,
                )
            }
            None => (MARKS, (x, y), (x, y + 1), 0),
        };

        let inside = |(x, y)| (y < card.bottom()).then_some(Position::new(x, y));
        let field = |row: u16, clip: u16| {
            let width = right.saturating_sub(x + indent + clip);
            Rect::new(x + indent, y + row, width, 1).intersection(card)
        };

        Self {
            avatar,
            status: inside(status),
            attachment: inside(attachment),
            senders: field(0, if dated { DATE_ROOM } else { 0 }),
            subject: field(1, clip),
            snippet: field(2, 0),
            right,
        }
    }
}
