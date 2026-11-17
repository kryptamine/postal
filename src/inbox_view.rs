use crate::{keymap::Move, message::Message};

/// The Message List's contents, selection and scroll position.
pub struct InboxView {
    /// `None` until the server has answered.
    pub messages: Option<Vec<Message>>,
    pub selected: usize,
    /// Index of the first visible row; kept by the widget so the selection stays on screen.
    pub offset: usize,
}

impl InboxView {
    pub const fn loading() -> Self {
        Self {
            messages: None,
            selected: 0,
            offset: 0,
        }
    }

    /// Shows `messages` newest `INTERNALDATE` first, with the newest selected.
    pub fn show(&mut self, mut messages: Vec<Message>) {
        messages.sort_by_key(|m| std::cmp::Reverse(m.received));
        self.messages = Some(messages);
        self.selected = 0;
        self.offset = 0;
    }

    pub fn apply(&mut self, movement: Move) {
        let last = self
            .messages
            .as_ref()
            .map_or(0, |m| m.len().saturating_sub(1));
        self.selected = match movement {
            Move::Down => (self.selected + 1).min(last),
            Move::Up => self.selected.saturating_sub(1),
            Move::First => 0,
            Move::Last => last,
        };
    }
}
