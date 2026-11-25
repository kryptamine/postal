use std::{collections::HashSet, iter};

use crate::{
    domain::body_key::BodyKey,
    domain::message::Message,
    ui::input::keymap::{By, Direction, Scroll},
    ui::preview::stack_line::StackLine,
};

/// Reading: the selected Thread as a stack of Messages, oldest first, each
/// folded to a line or unfolded to its header and body.
pub struct Reading {
    unfolded: HashSet<BodyKey>,
    /// Messages from others that joined the open Thread, marked `new`
    /// until the cursor reaches them.
    fresh: HashSet<BodyKey>,
    /// Every Message on a line of its own, from the first fold on.
    whole: bool,
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
            fresh: HashSet::new(),
            whole: false,
            scroll: 0,
            reveal: true,
            top: None,
            hold: false,
        })
    }

    pub fn unfolded(&self, key: BodyKey) -> bool {
        self.unfolded.contains(&key)
    }

    pub fn fresh(&self, key: BodyKey) -> bool {
        self.fresh.contains(&key)
    }

    /// Messages joined the stack: each Unread one from someone else unfolds
    /// and is marked `new`, and the top of the window stays where it was.
    pub fn joined<'a>(&mut self, joined: impl Iterator<Item = &'a Message>) {
        for message in joined.filter(|m| m.unread && !m.from_me) {
            self.unfolded.insert(message.body_key);
            self.fresh.insert(message.body_key);
        }

        self.hold = true;
    }

    /// The cursor has reached its Message: it is `new` no longer.
    pub fn reached(&mut self) {
        self.fresh.remove(&self.cursor);
    }

    /// Each line of `stack`: until the first fold, a run of two or more
    /// folded between the oldest and the newest is one line.
    pub fn lines<'a, I>(&self, stack: I) -> impl Iterator<Item = (&'a Message, StackLine)> + Clone
    where
        I: Iterator<Item = &'a Message> + Clone,
    {
        let last = stack.clone().count().saturating_sub(1);
        let middle = move |(index, message): &(usize, &Message)| {
            !self.whole && (1..last).contains(index) && !self.unfolded(message.body_key)
        };

        let mut rest = stack.enumerate();
        iter::from_fn(move || {
            let (index, message) = rest.next()?;
            if self.unfolded(message.body_key) {
                return Some((message, StackLine::Unfolded));
            }

            let run = if middle(&(index, message)) {
                1 + rest.clone().take_while(&middle).count()
            } else {
                1
            };

            if run < 2 {
                return Some((message, StackLine::Folded));
            }

            rest.nth(run - 2);
            Some((message, StackLine::Run(run)))
        })
    }

    /// Moves the cursor to the next or the previous line of `stack`.
    pub fn step<'a>(
        &mut self,
        direction: Direction,
        stack: impl Iterator<Item = &'a Message> + Clone,
    ) {
        if let Some(target) = self.neighbour(direction, stack) {
            self.cursor = target;
        }

        self.reveal = true;
    }

    /// The line of `stack` next to the cursor's, by its first Message.
    fn neighbour<'a>(
        &self,
        direction: Direction,
        stack: impl Iterator<Item = &'a Message> + Clone,
    ) -> Option<BodyKey> {
        let keys = self.lines(stack).map(|(message, _)| message.body_key);
        let mut pairs = keys.clone().zip(keys.skip(1));
        match direction {
            Direction::Down => pairs
                .find(|&(at, _)| at == self.cursor)
                .map(|(_, next)| next),
            Direction::Up => pairs
                .find(|&(_, at)| at == self.cursor)
                .map(|(before, _)| before),
        }
    }

    /// Folds the Message at the cursor, or unfolds it; on a line standing
    /// for several, puts each on its own line instead.
    pub fn fold<'a>(&mut self, stack: impl Iterator<Item = &'a Message> + Clone) {
        let cursor = self.cursor;
        let run = self
            .lines(stack)
            .any(|(message, line)| matches!(line, StackLine::Run(_)) && message.body_key == cursor);
        self.whole = true;
        if !run && !self.unfolded.remove(&cursor) {
            self.unfolded.insert(cursor);
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
