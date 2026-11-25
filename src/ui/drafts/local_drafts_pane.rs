use std::fmt::Write;

use chrono::Utc;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Clear, Widget},
};

use crate::{
    compose::accounts::Accounts, compose::local_draft::LocalDraft,
    compose::local_drafts::LocalDrafts, render::stack_str::StackStr, ui::frame::layout::PADDING,
    ui::frame::theme::Theme, ui::widgets::border, ui::widgets::centred::centred,
    ui::widgets::truncate,
};

/// The kind of a row, a word or an icon, and a gap after it.
const KIND: u16 = 8;
/// The recipients' share of what the kind leaves, in percent.
const TO_SHARE: u16 = 40;
/// The sender's share, with several Accounts, in percent.
const FROM_SHARE: u16 = 30;
/// The width of the count of files attached, at the right.
const ATTACHED_WIDTH: u16 = 5;
/// The width of a held Send's time left, before the count.
const HELD_WIDTH: u16 = 8;

/// Local Drafts over the whole body: a row per Draft or Outbox Message,
/// its kind, its sender with several Accounts, its recipients, its subject
/// how long a held Send has left and how many files it attaches.
pub struct LocalDraftsPane<'a> {
    pub theme: &'a Theme,
    pub local: &'a LocalDrafts,
    pub accounts: Accounts<'a>,
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
            [" Local Drafts "],
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
        let rest = list.width.saturating_sub(KIND);
        let several = self.accounts.0.len() > 1;
        let from_width = if several { rest * FROM_SHARE / 100 } else { 0 };
        let to_width = rest * TO_SHARE / 100;
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
            let from_x = list.x.saturating_add(KIND).min(list.right());
            if several {
                let account = self.accounts.place(row.account.as_deref());
                let from = account.map_or("", |account| self.accounts.email(account));
                truncate::draw(buf, cell(from_x, from_width.saturating_sub(1)), from, style);
            }

            let to_x = from_x.saturating_add(from_width).min(list.right());
            truncate::draw(buf, cell(to_x, to_width.saturating_sub(1)), &row.to, style);
            let subject_x = to_x.saturating_add(to_width).min(list.right());
            let tail = Rect::new(subject_x, y, list.right() - subject_x, 1);
            self.tail(buf, tail, row, style);
        }
    }
}

impl LocalDraftsPane<'_> {
    /// The subject in `area`, then how long a held Send has left and the
    /// count of files attached, at its right.
    fn tail(&self, buf: &mut Buffer, area: Rect, row: &LocalDraft, style: Style) {
        let cell = |x: u16, width: u16| Rect::new(x, area.y, width.min(area.right() - x), 1);
        let muted = self.theme.muted.patch(style);
        let attached_x = area.right().saturating_sub(ATTACHED_WIDTH).max(area.x);
        let left = row
            .hold_until
            .and_then(|until| (until - Utc::now()).to_std().ok());
        let held_x = match left {
            Some(_) => attached_x.saturating_sub(HELD_WIDTH).max(area.x),
            None => attached_x,
        };

        let subject_width = held_x.saturating_sub(area.x).saturating_sub(1);
        truncate::draw(buf, cell(area.x, subject_width), &row.subject, style);
        if let Some(left) = left {
            let seconds = left.as_secs() + u64::from(left.subsec_nanos() > 0);
            let mut text = StackStr::<24>::new();
            // Any u64 fits, so the write cannot fail.
            write!(text, "in {seconds}s").ok();
            truncate::draw(buf, cell(held_x, HELD_WIDTH), text.as_str(), muted);
        }

        if row.attached > 0 {
            let count = format!("{} {}", self.theme.icons.attachment(), row.attached);
            truncate::draw(buf, cell(attached_x, ATTACHED_WIDTH), &count, muted);
        }
    }
}
