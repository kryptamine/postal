use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Clear, Widget},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    ui::frame::theme::Theme,
    ui::list::completion::{Completion, Suggestion},
    ui::widgets::{scrollbar::scrollbar, truncate},
};

const GAP: u16 = 2;
const BAR: u16 = 1;

/// The menu above the Query line, its border's left edge at the column
/// before the word completed: each suggestion, then its detail muted, the
/// one written into the Query selected.
pub struct CompletionMenu<'a> {
    pub theme: &'a Theme,
    pub completion: &'a Completion,
    /// The column the word completed starts at.
    pub x: u16,
    /// The row the menu's bottom border sits on top of.
    pub above: u16,
}

impl Widget for CompletionMenu<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let suggestions = self.completion.suggestions();
        if suggestions.is_empty() {
            return;
        }

        let widest = |widths: &mut dyn Iterator<Item = usize>| {
            u16::try_from(widths.max().unwrap_or(0)).unwrap_or(u16::MAX)
        };
        let text = widest(&mut suggestions.iter().map(|s| s.text.width()));
        let detail = widest(&mut suggestions.iter().map(|s| s.detail.width()));
        let inner = text.saturating_add(if detail > 0 { GAP + detail } else { 0 });
        let width = inner.saturating_add(4).min(area.width);
        let rows = u16::try_from(suggestions.len()).unwrap_or(u16::MAX);
        let room = self.above.saturating_sub(area.y);
        let height = rows.saturating_add(2).min(room);
        if height < 3 || width < 5 {
            return;
        }

        let x = self
            .x
            .saturating_sub(2)
            .min(area.right().saturating_sub(width))
            .max(area.x);
        let menu = Rect::new(x, self.above - height, width, height);
        Clear.render(menu, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(menu, buf);

        let fitting = usize::from(height - 2);
        let total = suggestions.len();
        let scrollable = total > fitting;
        let selected = self.completion.selected();
        let offset = selected.map_or(0, |at| (at + 1).saturating_sub(fitting));
        let shown = suggestions.iter().enumerate().skip(offset).take(fitting);
        let row_width = menu.width - 2 - if scrollable { BAR } else { 0 };
        for ((index, suggestion), y) in shown.zip(menu.y + 1..) {
            let row = Rect::new(menu.x + 1, y, row_width, 1);
            let style = if selected == Some(index) {
                self.theme.selected
            } else {
                Style::new()
            };

            let muted = style.patch(self.theme.muted);
            draw_row(buf, row, suggestion, text, (style, muted));
        }

        let track = Rect::new(menu.right().saturating_sub(2), menu.y + 1, BAR, height - 2);
        scrollbar(buf, track, (offset, total), self.theme.muted);
    }
}

/// `suggestion` in `row`: its text in a column `text` wide, then its detail.
fn draw_row(
    buf: &mut Buffer,
    row: Rect,
    suggestion: &Suggestion,
    text: u16,
    (style, muted): (Style, Style),
) {
    buf.set_style(row, style);
    let right = row.right().saturating_sub(1);
    let at = row.x + 1;
    let width = text.min(right.saturating_sub(at));
    truncate::draw(buf, Rect::new(at, row.y, width, 1), &suggestion.text, style);
    let at = at.saturating_add(text + GAP);
    let width = right.saturating_sub(at);
    truncate::draw(
        buf,
        Rect::new(at, row.y, width, 1),
        &suggestion.detail,
        muted,
    );
}
