use chrono::{DateTime, FixedOffset, Local, Offset, Utc};

use crate::{
    inbox::Inbox,
    keymap::{By, Direction, Move, Scroll},
    message::Message,
};

/// The Message List's contents, selection and scroll position.
pub struct InboxView {
    /// `None` until the Cache or the server has rows to show. Their dates
    /// are local, each at the offset in force when it was received.
    pub messages: Option<Vec<Message>>,
    /// The server's count over the whole Inbox.
    pub unread: usize,
    pub selected: usize,
    /// Index of the first visible card; kept by the widget so the selection stays on screen.
    pub offset: usize,
    /// The Preview's first row shown; back to 0 whenever the selection
    /// moves to another Message, and kept in range by the Preview widget.
    pub preview_scroll: usize,
    /// The local offset as of the last `show`: chrono's `Local` rereads the
    /// time zone every second, and that allocates inside a frame.
    zone: FixedOffset,
}

impl InboxView {
    pub fn loading() -> Self {
        Self {
            messages: None,
            unread: 0,
            selected: 0,
            offset: 0,
            preview_scroll: 0,
            zone: Utc.fix(),
        }
    }

    /// Shows rows already ordered newest first. The selection stays on the
    /// same Message, or on the same index when that Message is gone.
    pub fn show(&mut self, mut inbox: Inbox) {
        let selected = self.selected_uid();
        let last = inbox.messages.len().saturating_sub(1);
        self.selected = selected
            .and_then(|uid| inbox.messages.iter().position(|m| m.uid == uid))
            .unwrap_or_else(|| self.selected.min(last));
        for message in &mut inbox.messages {
            message.received = message.received.with_timezone(&Local).fixed_offset();
        }

        self.messages = Some(inbox.messages);
        if self.selected_uid() != selected {
            self.preview_scroll = 0;
        }

        self.unread = inbox.unread;
        self.zone = Local::now().offset().fix();
    }

    pub fn now(&self) -> DateTime<FixedOffset> {
        Utc::now().with_timezone(&self.zone)
    }

    pub fn apply(&mut self, movement: Move) {
        let last = self
            .messages
            .as_ref()
            .map_or(0, |m| m.len().saturating_sub(1));
        let selected = match movement {
            Move::Down => (self.selected + 1).min(last),
            Move::Up => self.selected.saturating_sub(1),
            Move::First => 0,
            Move::Last => last,
        };

        if selected != self.selected {
            self.selected = selected;
            self.preview_scroll = 0;
        }
    }

    /// Past the last page is fine: the Preview widget clamps it.
    pub const fn scroll(&mut self, scroll: Scroll, half_page: usize) {
        let rows = match scroll.by {
            By::Line => 1,
            By::HalfPage => half_page,
            By::End => usize::MAX,
        };

        self.preview_scroll = match scroll.direction {
            Direction::Down => self.preview_scroll.saturating_add(rows),
            Direction::Up => self.preview_scroll.saturating_sub(rows),
        };
    }

    pub fn selected_uid(&self) -> Option<u32> {
        self.messages
            .as_ref()
            .and_then(|m| m.get(self.selected))
            .map(|m| m.uid)
    }
}
