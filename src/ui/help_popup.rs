use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    ui::keymap::{self, Binding},
    ui::mode::Mode,
    ui::theme::Theme,
};

const TITLE: &str = " Help ";
const GAP: u16 = 2;
/// The border and one column or row of padding inside it, each side.
const FRAME: u16 = 4;

/// The keymap of `mode`, grouped as `keymap::bindings` lists it, centred over `area`:
/// a heading per group, then key, aliases and description per row.
pub struct HelpPopup<'a> {
    pub theme: &'a Theme,
    pub mode: Mode,
    pub signed_out: bool,
}

impl Widget for HelpPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bindings = keymap::bindings(self.mode, self.signed_out);
        let keys = column(bindings.clone(), |b| b.keys);
        let aliases = column(bindings.clone(), |b| b.aliases);
        let descriptions = column(bindings.clone(), |b| b.description);
        let width = keys + GAP + aliases + GAP + descriptions + FRAME;
        let popup = centre(area, width, rows(bindings.clone()) + FRAME);
        Clear.render(popup, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(popup, buf);
        let room = usize::from(popup.width.saturating_sub(2));
        buf.set_stringn(popup.x + 1, popup.y, TITLE, room, self.theme.title);

        let inner = Rect {
            x: popup.x.saturating_add(2),
            y: popup.y.saturating_add(2),
            width: popup.width.saturating_sub(FRAME),
            height: popup.height.saturating_sub(FRAME),
        };

        let mut lines = inner.y..inner.bottom();
        let mut group = "";
        for binding in bindings {
            if binding.group != group {
                if !group.is_empty() {
                    lines.next();
                }

                group = binding.group;
                let Some(y) = lines.next() else { return };
                cell(buf, (inner.x, y), inner.right(), group, self.theme.title);
            }

            let Some(y) = lines.next() else { return };
            let cells = [
                (0, binding.keys, self.theme.key),
                (keys + GAP, binding.aliases, self.theme.key),
                (
                    keys + GAP + aliases + GAP,
                    binding.description,
                    self.theme.muted,
                ),
            ];

            for (offset, text, style) in cells {
                let x = inner.x.saturating_add(offset);
                cell(buf, (x, y), inner.right(), text, style);
            }
        }
    }
}

/// The widest of one column over `bindings`.
fn column<'a>(
    bindings: impl Iterator<Item = &'a Binding>,
    text: impl Fn(&Binding) -> &'static str,
) -> u16 {
    bindings
        .map(|b| u16::try_from(text(b).width()).unwrap_or(u16::MAX))
        .max()
        .unwrap_or(0)
}

/// A row per binding and per group heading, and a blank row between groups.
fn rows<'a>(bindings: impl Iterator<Item = &'a Binding> + Clone) -> u16 {
    let groups = bindings
        .clone()
        .zip(bindings.clone().skip(1))
        .filter(|(a, b)| a.group != b.group)
        .count()
        + 1;
    u16::try_from(bindings.count() + groups * 2 - 1).unwrap_or(u16::MAX)
}

fn centre(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn cell(buf: &mut Buffer, (x, y): (u16, u16), right: u16, text: &str, style: Style) {
    if x < right {
        buf.set_stringn(x, y, text, usize::from(right - x), style);
    }
}
