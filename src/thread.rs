use std::collections::HashMap;

use crate::message::Message;

/// What tells one Thread from another across reloads of the Message List.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// Each names its Account first: a Thread never spans Accounts, and their
/// IDs and body keys may repeat.
pub enum Key {
    Gmail(usize, u64),
    /// A Message with no thread ID, a Thread of its own, by body key: UIDs
    /// repeat across Folders.
    Lone(usize, i64),
}

impl Key {
    pub const fn of(message: &Message) -> Self {
        match message.thread {
            Some(id) => Self::Gmail(message.account, id),
            None => Self::Lone(message.account, message.body_key),
        }
    }
}

/// One Message of a Thread's stack, by its index among the shown Folder's
/// Messages or among those found in other Folders.
#[derive(Clone, Copy)]
pub enum Place {
    Here(usize),
    Elsewhere(usize),
}

/// One row of the Message List: the Messages of a Thread in the shown Folder.
pub struct Thread {
    pub key: Key,
    /// Indices into the Folder's Messages, newest first; never empty.
    pub messages: Vec<usize>,
    /// The whole Thread as Reading shows it, these Messages and those found
    /// in other Folders, oldest first.
    pub stack: Vec<Place>,
    /// One Message per distinct sender name, in order of first appearance.
    pub senders: Vec<usize>,
    pub unread: bool,
    /// Any Message of it has `\\Answered`, here or elsewhere.
    pub answered: bool,
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

/// Groups `messages`, newest first, into Threads ordered by their newest
/// arrival; each of `elsewhere` joins the stack of its Thread, if shown.
pub fn threads(messages: &[Message], elsewhere: &[Message]) -> Vec<Thread> {
    let mut threads: Vec<Thread> = Vec::new();
    let mut by_key = HashMap::new();
    for (index, message) in messages.iter().enumerate() {
        let key = Key::of(message);
        let at = *by_key.entry(key).or_insert_with(|| {
            threads.push(Thread {
                key,
                messages: Vec::new(),
                stack: Vec::new(),
                senders: Vec::new(),
                unread: false,
                answered: false,
            });
            threads.len() - 1
        });

        let thread = &mut threads[at];
        thread.messages.push(index);
        thread.unread |= message.unread;
        thread.answered |= message.answered;
    }

    for (index, message) in elsewhere.iter().enumerate() {
        if let Some(&at) = by_key.get(&Key::of(message)) {
            threads[at].stack.push(Place::Elsewhere(index));
            threads[at].answered |= message.answered;
        }
    }

    let received = |place: &Place| match *place {
        Place::Here(i) => messages[i].received,
        Place::Elsewhere(i) => elsewhere[i].received,
    };

    for thread in &mut threads {
        let here = thread.messages.iter().rev().map(|&i| Place::Here(i));
        thread.stack.splice(0..0, here);
        // Stable: of one date, the shown Folder's go first.
        thread.stack.sort_by_key(received);
        for &index in thread.messages.iter().rev() {
            let name = sender(&messages[index]);
            if !thread.senders.iter().any(|&s| sender(&messages[s]) == name) {
                thread.senders.push(index);
            }
        }
    }

    threads
}
