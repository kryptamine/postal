use std::collections::HashSet;

use crate::{
    keymap::{By, Direction, Scroll},
    message::Message,
};

/// Reading: the selected Thread as a stack of Messages, oldest first, each
/// folded to a line or unfolded to its header and body.
pub struct Reading {
    unfolded: HashSet<u32>,
    /// The UID of the Message at the cursor; one gone from the Thread
    /// falls back to the newest as the stack is laid out.
    pub cursor: u32,
    /// The stack's first row shown, kept in range as it is laid out.
    pub scroll: usize,
    /// Until the next scroll, each frame brings the cursor's Message into view.
    pub reveal: bool,
}

impl Reading {
    /// The newest and every Unread Message unfolded, the cursor on the
    /// first Unread, else the newest; `None` for an empty `stack`.
    pub fn open<'a>(stack: impl Iterator<Item = &'a Message> + Clone) -> Option<Self> {
        let newest = stack.clone().last()?;
        let mut unread = stack.filter(|m| m.unread).peekable();
        Some(Self {
            cursor: unread.peek().copied().unwrap_or(newest).uid,
            unfolded: unread.chain([newest]).map(|m| m.uid).collect(),
            scroll: 0,
            reveal: true,
        })
    }

    pub fn unfolded(&self, uid: u32) -> bool {
        self.unfolded.contains(&uid)
    }

    /// Moves the cursor to the next or the previous Message of `stack`.
    pub fn step<'a>(&mut self, direction: Direction, mut stack: impl Iterator<Item = &'a Message>) {
        let mut before = None;
        while let Some(message) = stack.next() {
            if message.uid == self.cursor {
                let target = match direction {
                    Direction::Down => stack.next(),
                    Direction::Up => before,
                };

                if let Some(target) = target {
                    self.cursor = target.uid;
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
