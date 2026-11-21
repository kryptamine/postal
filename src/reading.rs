use std::collections::HashSet;

use crate::{
    body_key::BodyKey,
    keymap::{By, Direction, Scroll},
    message::Message,
};

/// Reading: the selected Thread as a stack of Messages, the oldest then the
/// rest newest first, each folded to a line or unfolded to its header and body.
pub struct Reading {
    unfolded: HashSet<BodyKey>,
    /// The body key of the Message at the cursor, which tells apart Messages
    /// of different Folders; one gone from the Thread falls back to the
    /// newest as the stack is laid out.
    pub cursor: BodyKey,
    /// The stack's first row shown, kept in range as it is laid out.
    pub scroll: usize,
    /// Until the next scroll, each frame brings the cursor's Message into view.
    pub reveal: bool,
    /// The Message at the top of the window, by body key, and the rows into
    /// it, as last laid out.
    pub top: Option<(BodyKey, usize)>,
    /// Set when Messages joined the stack: the next layout puts `top` back.
    pub hold: bool,
}

impl Reading {
    /// The newest and every Unread Message unfolded, the cursor on the
    /// oldest Unread, else the newest; `None` for an empty `stack`.
    pub fn open<'a>(stack: impl Iterator<Item = &'a Message> + Clone) -> Option<Self> {
        let newest = stack.clone().max_by_key(|m| m.received)?;
        let unread = stack.filter(|m| m.unread);
        let oldest = unread.clone().min_by_key(|m| m.received);
        Some(Self {
            cursor: oldest.unwrap_or(newest).body_key,
            unfolded: unread.chain([newest]).map(|m| m.body_key).collect(),
            scroll: 0,
            reveal: true,
            top: None,
            hold: false,
        })
    }

    pub fn unfolded(&self, key: BodyKey) -> bool {
        self.unfolded.contains(&key)
    }

    /// Moves the cursor to the next or the previous Message of `stack`.
    pub fn step<'a>(&mut self, direction: Direction, mut stack: impl Iterator<Item = &'a Message>) {
        let mut before = None;
        while let Some(message) = stack.next() {
            if message.body_key == self.cursor {
                let target = match direction {
                    Direction::Down => stack.next(),
                    Direction::Up => before,
                };

                if let Some(target) = target {
                    self.cursor = target.body_key;
                }

                break;
            }

            before = Some(message);
        }

        self.reveal = true;
    }

    /// Folds the Message at the cursor, or unfolds it.
    pub fn fold(&mut self) {
        if !self.unfolded.remove(&self.cursor) {
            self.unfolded.insert(self.cursor);
        }

        self.reveal = true;
    }

    /// Past the last page is fine: laying the stack out clamps it.
    pub const fn scroll(&mut self, scroll: Scroll, half_page: usize) {
        let rows = match scroll.by {
            By::Line => 1,
            By::HalfPage => half_page,
            By::End => usize::MAX,
        };

        self.scroll = match scroll.direction {
            Direction::Down => self.scroll.saturating_add(rows),
            Direction::Up => self.scroll.saturating_sub(rows),
        };

        self.reveal = false;
    }
}
