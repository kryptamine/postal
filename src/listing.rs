use chrono::Local;

use crate::{
    inbox::Inbox,
    message::Message,
    thread::{self, Key, Place, Thread},
};

/// Messages read from the Cache, grouped into the Threads a list shows.
pub struct Listing {
    /// Newest first; dates are local, each at the offset in force when it
    /// was received.
    pub messages: Vec<Message>,
    /// Messages of their Threads found in other Folders, oldest first.
    pub elsewhere: Vec<Message>,
    pub threads: Vec<Thread>,
}

impl Listing {
    pub fn new(mut inbox: Inbox) -> Self {
        for message in inbox.messages.iter_mut().chain(&mut inbox.elsewhere) {
            message.received = message.received.with_timezone(&Local).fixed_offset();
        }

        Self {
            threads: thread::threads(&inbox.messages, &inbox.elsewhere),
            messages: inbox.messages,
            elsewhere: inbox.elsewhere,
        }
    }

    pub fn newest(&self, thread: &Thread) -> Option<&Message> {
        self.messages.get(thread.newest())
    }

    pub fn find(&self, key: Key) -> Option<usize> {
        self.threads.iter().position(|t| t.key == key)
    }

    /// The Messages of `thread`, those found in other Folders too: the
    /// oldest, then the rest newest first.
    pub fn stack<'a>(
        &'a self,
        thread: Option<&'a Thread>,
    ) -> impl Iterator<Item = &'a Message> + Clone {
        let places = thread.map_or(&[][..], |t| t.stack.as_slice());
        places.iter().filter_map(|&place| match place {
            Place::Here(i) => self.messages.get(i),
            Place::Elsewhere(i) => self.elsewhere.get(i),
        })
    }
}
