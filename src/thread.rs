use std::collections::HashMap;

use crate::message::Message;

/// What tells one Thread from another across reloads of the Message List.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Gmail(u64),
    /// A Message with no thread ID, a Thread of its own.
    Lone(u32),
}

impl Key {
    pub const fn of(message: &Message) -> Self {
        match message.thread {
            Some(id) => Self::Gmail(id),
            None => Self::Lone(message.uid),
        }
    }
}

/// One row of the Message List: the Messages of a Thread in the shown Folder.
pub struct Thread {
    pub key: Key,
    /// Indices into the Folder's Messages, newest first; never empty.
    pub messages: Vec<usize>,
    /// One Message per distinct sender name, in order of first appearance.
    pub senders: Vec<usize>,
    pub unread: bool,
}

impl Thread {
    pub fn newest(&self) -> usize {
        self.messages[0]
    }

    pub fn oldest(&self) -> usize {
        self.messages[self.messages.len() - 1]
    }
}

/// The name a Message's sender goes by in the Message List.
pub fn sender(message: &Message) -> &str {
    if message.from_me {
        "me"
    } else {
        &message.sender
    }
}

/// Groups `messages`, newest first, into Threads ordered by their newest arrival.
pub fn threads(messages: &[Message]) -> Vec<Thread> {
    let mut threads: Vec<Thread> = Vec::new();
    let mut by_key = HashMap::new();
    for (index, message) in messages.iter().enumerate() {
        let key = Key::of(message);
        let at = *by_key.entry(key).or_insert_with(|| {
            threads.push(Thread {
                key,
                messages: Vec::new(),
                senders: Vec::new(),
                unread: false,
            });
            threads.len() - 1
        });

        let thread = &mut threads[at];
        thread.messages.push(index);
        thread.unread |= message.unread;
    }

    for thread in &mut threads {
        for &index in thread.messages.iter().rev() {
            let name = sender(&messages[index]);
            if !thread.senders.iter().any(|&s| sender(&messages[s]) == name) {
                thread.senders.push(index);
            }
        }
    }

    threads
}
