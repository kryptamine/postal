use std::fmt::Write;

use ratatui::{
    buffer::Buffer,
    style::{Color, Modifier, Style},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    domain::message::Message, render::rgb::Rgb, render::stack_str::StackStr,
    ui::frame::theme::Theme,
};

pub const WIDTH: u16 = 4;
/// How much of the hue the tile takes over the terminal's background:
/// enough to tell senders apart, little enough to sit behind the text.
const TINT: f32 = 0.28;

/// Spread round the wheel at one brightness, so no sender stands out.
const HUES: [Rgb; 9] = [
    rgb(225, 95, 110),
    rgb(230, 140, 70),
    rgb(210, 170, 60),
    rgb(80, 170, 110),
    rgb(50, 165, 160),
    rgb(80, 140, 220),
    rgb(120, 120, 220),
    rgb(170, 110, 210),
    rgb(215, 100, 170),
];

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

/// The tile of `message`'s sender at `(x, y)`, down `rows` rows of at most
/// three: half blocks above and below the initials make it two rows tall
/// to the eye, the initials centred. `selected` is the card's selection.
pub fn draw(
    buf: &mut Buffer,
    (x, y, rows): (u16, u16, u16),
    message: &Message,
    selected: bool,
    theme: &Theme,
) {
    let (edge, face) = styles(address(&message.from), selected, theme);
    let initials = initials(message);
    let mut face_text = StackStr::<32>::new();
    let room = usize::from(WIDTH).saturating_sub(initials.as_str().width());
    let left = room / 2;
    write!(
        face_text,
        "{:left$}{}{:right$}",
        "",
        initials.as_str(),
        "",
        right = room - left
    )
    .ok();
    let tile = [("▄▄▄▄", edge), (face_text.as_str(), face), ("▀▀▀▀", edge)];
    for (dy, (text, style)) in (0..rows.min(3)).zip(tile) {
        buf.set_stringn(x, y + dy, text, usize::from(WIDTH), style);
    }
}

/// The half blocks' style and the initials'. Under `NO_COLOR` the tile is
/// the card's text colour: the face reverses the card, the edges keep it.
fn styles(address: &str, selected: bool, theme: &Theme) -> (Style, Style) {
    let Some(colours) = theme.avatar_colours() else {
        let face = Style::new().add_modifier(Modifier::BOLD);
        let face = if selected {
            face.remove_modifier(Modifier::REVERSED)
        } else {
            face.add_modifier(Modifier::REVERSED)
        };

        return (Style::new(), face);
    };

    let hue = hue(address);
    let tile = colours.terminal().mix(hue, TINT);
    let ink = colours.text(hue, Some(tile));
    // The selection is a reversed named colour; the edges' blank half sits on it.
    let ground = if selected { theme.selected.fg } else { None };
    let edge = Style::new()
        .fg(colour(tile))
        .bg(ground.unwrap_or(Color::Reset))
        .remove_modifier(Modifier::REVERSED);
    let face = Style::new()
        .fg(colour(ink))
        .bg(colour(tile))
        .add_modifier(Modifier::BOLD)
        .remove_modifier(Modifier::REVERSED);
    (edge, face)
}

const fn colour(Rgb { r, g, b }: Rgb) -> Color {
    Color::Rgb(r, g, b)
}

/// The address of a `From` as `Name <address>` shows it, or the whole of it.
fn address(from: &str) -> &str {
    let inner = from
        .rsplit_once('<')
        .and_then(|(_, rest)| rest.split_once('>'))
        .map_or(from, |(address, _)| address);
    inner.trim()
}

/// FNV-1a over the address without case: fixed, so a sender keeps its hue
/// on every card and every run, as `RandomState` would not.
fn hue(address: &str) -> Rgb {
    let fnv = address.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b.to_ascii_lowercase())).wrapping_mul(0x0100_0000_01b3)
    });
    let count = u64::try_from(HUES.len()).unwrap_or(1);
    HUES[usize::try_from(fnv % count).unwrap_or_default()]
}

/// The first letters of the first two words of the sender's name, else of
/// the address's local part; one word gives its first two letters.
fn initials(message: &Message) -> StackStr<16> {
    let name = message.sender.trim();
    let name = if name.is_empty() {
        let address = address(&message.from);
        address.split('@').next().unwrap_or(address)
    } else {
        name
    };

    let mut words = name
        .split(|c: char| c.is_whitespace() || matches!(c, '.' | '@' | '_' | '-'))
        .map(|word| word.chars().filter(|c| c.is_alphanumeric()))
        .filter_map(|mut letters| letters.next().map(|first| (first, letters)));
    let pair = match (words.next(), words.next()) {
        (Some((first, _)), Some((second, _))) => [Some(first), Some(second)],
        (Some((first, mut rest)), None) => [Some(first), rest.next()],
        (None, _) => [None, None],
    };

    let mut letters = StackStr::new();
    for letter in pair.into_iter().flatten() {
        let mut upper = StackStr::<8>::new();
        write!(upper, "{}", letter.to_uppercase()).ok();
        // Two wide letters already fill the tile.
        if letters.as_str().width() + upper.as_str().width() > usize::from(WIDTH) {
            break;
        }

        letters.write_str(upper.as_str()).ok();
    }

    letters
}
