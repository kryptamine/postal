use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Clear, Widget},
};

use crate::{
    border, centred::centred, layout::PADDING, local_drafts::LocalDrafts, theme::Theme, truncate,
};

/// The kind of a row, a word or an icon, and a gap after it.
const KIND: u16 = 8;
/// The recipients' share of what the kind leaves, in percent.
const TO_SHARE: u16 = 40;

/// Local Drafts over the whole body: a row per Draft or Outbox Message,
/// its kind, its recipients and its subject.
pub struct LocalDraftsPane<'a> {
    pub theme: &'a Theme,
    pub local: &'a LocalDrafts,
}

impl Widget for LocalDraftsPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(area, buf);
        border::label(
            buf,
            area,
            (area.x + 1, area.y),
            &[" Local Drafts "],
            self.theme.title,
        );
        let list = Rect {
            x: area.x.saturating_add(1 + PADDING),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2 * (1 + PADDING)),
            height: area.height.saturating_sub(2),
        };

        if self.local.rows().is_empty() {
            return centred("no Drafts", list, buf);
        }

        let fitting = usize::from(list.height);
        let offset = (self.local.selected() + 1).saturating_sub(fitting);
        let rows = self.local.rows().iter().enumerate().skip(offset);
        let to_width = list.width.saturating_sub(KIND) * TO_SHARE / 100;
        for ((index, row), y) in rows.take(fitting).zip(list.y..) {
            let style = if index == self.local.selected() {
                self.theme.selected
            } else {
                Style::new()
            };

            buf.set_style(
                Rect::new(area.x + 1, y, area.width.saturating_sub(2), 1),
                style,
            );
            let kind = self.theme.icons.local(row.queued);
            let cell = |x: u16, width: u16| Rect::new(x, y, width.min(list.right() - x), 1);
            truncate::draw(buf, cell(list.x, KIND), kind, self.theme.muted.patch(style));
            let to_x = list.x.saturating_add(KIND).min(list.right());
            truncate::draw(buf, cell(to_x, to_width.saturating_sub(1)), &row.to, style);
            let subject_x = to_x.saturating_add(to_width).min(list.right());
            truncate::draw(buf, cell(subject_x, list.right()), &row.subject, style);
        }
    }
}
