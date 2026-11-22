use std::collections::HashMap;

use chrono::Local;

use crate::{
    domain::inbox::Inbox,
    domain::message::Message,
    domain::pool::Pool,
    domain::thread::{self, Key, Place, Thread},
};

/// Messages read from the Cache, grouped into the Threads a list shows;
/// filled again in place, its buffers reused.
pub struct Listing {
    /// Newest first; dates are local, each at the offset in force when it
    /// was received.
    messages: Pool<Message>,
    /// Messages of their Threads found in other Folders, oldest first.
    elsewhere: Pool<Message>,
    threads: Pool<Thread>,
    by_key: HashMap<Key, usize>,
}

impl Listing {
    pub fn new(mut inbox: Inbox) -> Self {
        for message in inbox.messages.iter_mut().chain(&mut inbox.elsewhere) {
            message.received = message.received.with_timezone(&Local).fixed_offset();
        }

        let mut listing = Self {
            messages: Pool::from(inbox.messages),
            elsewhere: Pool::from(inbox.elsewhere),
            threads: Pool::new(),
            by_key: HashMap::new(),
        };

        listing.regroup();
        listing
    }

    pub fn empty() -> Self {
        Self::new(Inbox {
            messages: Vec::new(),
            elsewhere: Vec::new(),
        })
    }

    /// Fills it anew: `fill` writes the Messages into the slots of the
    /// shown Folder's, then of those elsewhere, with dates already local.
    pub fn refill<E>(
        &mut self,
        fill: impl FnOnce(&mut Pool<Message>, &mut Pool<Message>) -> Result<(), E>,
    ) -> Result<(), E> {
        self.messages.clear();
        self.elsewhere.clear();
        let filled = fill(&mut self.messages, &mut self.elsewhere);
        self.regroup();
        filled
    }

    fn regroup(&mut self) {
        let (messages, elsewhere) = (self.messages.as_slice(), self.elsewhere.as_slice());
        thread::group(&mut self.threads, &mut self.by_key, messages, elsewhere);
    }

    pub fn messages(&self) -> &[Message] {
        self.messages.as_slice()
    }

    /// In order of their first Message among `messages`.
    pub fn threads(&self) -> &[Thread] {
        self.threads.as_slice()
    }

    pub fn newest(&self, thread: &Thread) -> Option<&Message> {
        self.messages().get(thread.newest())
    }

    pub fn find(&self, key: Key) -> Option<usize> {
        self.by_key.get(&key).copied()
    }

    /// The Messages of `thread`, those found in other Folders too, oldest
    /// first.
    pub fn stack<'a>(
        &'a self,
        thread: Option<&'a Thread>,
    ) -> impl Iterator<Item = &'a Message> + Clone {
        let places = thread.map_or(&[][..], |t| t.stack.as_slice());
        places.iter().filter_map(|&place| match place {
            Place::Here(i) => self.messages().get(i),
            Place::Elsewhere(i) => self.elsewhere.as_slice().get(i),
        })
    }
}
