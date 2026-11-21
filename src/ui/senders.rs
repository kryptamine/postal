use unicode_width::UnicodeWidthStr;

use crate::{
    domain::message::Message,
    domain::thread::{self, Thread},
    ui::search::SENDER_GAP,
};

/// Between the first and the last sender when those in between are left out.
const ELISION: &str = " … ";

/// A Thread's sender names as its card shows them in `width` columns: all
/// when they fit, else the first and the last around `ELISION`, else the
/// first alone, cut.
pub struct Senders<'a> {
    messages: &'a [Message],
    thread: &'a Thread,
    ends_only: bool,
}

impl<'a> Senders<'a> {
    pub fn fit(messages: &'a [Message], thread: &'a Thread, width: usize) -> Self {
        let mut senders = Self {
            messages,
            thread,
            ends_only: false,
        };

        let count = thread.senders.len();
        let all = (0..count).map(|n| senders.name(n).width()).sum::<usize>()
            + SENDER_GAP.width() * count.saturating_sub(1);
        senders.ends_only = all > width && count > 2;
        senders
    }

    /// Each name shown, with its place among the Thread's senders; a gap
    /// goes before every one but the first.
    pub fn shown(&self) -> impl Iterator<Item = (usize, &'a str)> + '_ {
        let count = self.thread.senders.len();
        let last = count.saturating_sub(1);
        (0..count)
            .filter(move |&n| !self.ends_only || n == 0 || n == last)
            .map(|n| (n, self.name(n)))
    }

    pub const fn gap(&self) -> &'static str {
        if self.ends_only { ELISION } else { SENDER_GAP }
    }

    /// The sender `n`th in order of first appearance.
    pub fn name(&self, n: usize) -> &'a str {
        thread::sender(&self.messages[self.thread.senders[n]])
    }
}
