use crate::{inbox::Inbox, keymap::Move, message::Message};

/// The Message List's contents, selection and scroll position.
pub struct InboxView {
    /// `None` until the Cache or the server has rows to show.
    pub messages: Option<Vec<Message>>,
    /// The server's count over the whole Inbox.
    pub unread: usize,
    pub selected: usize,
    /// Index of the first visible card; kept by the widget so the selection stays on screen.
    pub offset: usize,
}

impl InboxView {
    pub const fn loading() -> Self {
        Self {
            messages: None,
            unread: 0,
            selected: 0,
            offset: 0,
        }
    }

    /// Shows rows already ordered newest first. The selection stays on the
    /// same Message, or on the same index when that Message is gone.
    pub fn show(&mut self, inbox: Inbox) {
        let selected = self
            .messages
            .as_ref()
            .and_then(|m| m.get(self.selected))
            .map(|m| m.uid);
        let last = inbox.messages.len().saturating_sub(1);
        self.selected = selected
            .and_then(|uid| inbox.messages.iter().position(|m| m.uid == uid))
            .unwrap_or_else(|| self.selected.min(last));
        self.messages = Some(inbox.messages);
        self.unread = inbox.unread;
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
