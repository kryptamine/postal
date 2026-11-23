use std::{cmp::Reverse, collections::HashMap};

use crate::{
    domain::body_key::BodyKey, domain::message::Message, domain::per_account::AccountId,
    domain::pool::Pool, domain::root_id::RootId, domain::thread_id::ThreadId,
};

/// What tells one Thread from another across reloads of the Message List.
/// Each names its Account first: a Thread never spans Accounts, and their
/// IDs and body keys may repeat. A command names it by address (`Key<String>`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key<A = AccountId> {
    Gmail(A, ThreadId),
    Root(A, RootId),
    /// A Message with neither, a Thread of its own, by body key: UIDs
    /// repeat across Folders.
    Lone(A, BodyKey),
}

impl Key {
    pub const fn of(message: &Message) -> Self {
        Self::new(
            message.account,
            message.thread,
            message.root,
            message.body_key,
        )
    }
}

impl<A> Key<A> {
    /// Gmail's Thread first, then the root of `References`, else the body.
    pub const fn new(
        account: A,
        thread: Option<ThreadId>,
        root: Option<RootId>,
        body: BodyKey,
    ) -> Self {
        match (thread, root) {
            (Some(id), _) => Self::Gmail(account, id),
            (None, Some(root)) => Self::Root(account, root),
            (None, None) => Self::Lone(account, body),
        }
    }

    pub const fn account(&self) -> &A {
        match self {
            Self::Gmail(account, _) | Self::Root(account, _) | Self::Lone(account, _) => account,
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
    /// Any Message of it has an attachment, here or elsewhere.
    pub attachments: bool,
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

/// Groups `messages`, each Thread's newest first, into `threads` in order
/// of their first Message, each found by its key in `by_key`; each of `elsewhere`
/// joins the stack of its Thread, if shown. Reuses the buffers it is given.
pub fn group(
    threads: &mut Pool<Thread>,
    by_key: &mut HashMap<Key, usize>,
    messages: &[Message],
    elsewhere: &[Message],
) {
    threads.clear();
    by_key.clear();
    for (index, message) in messages.iter().enumerate() {
        let key = Key::of(message);
        let at = *by_key.entry(key).or_insert_with(|| open(threads, key));
        let thread = &mut threads.as_mut_slice()[at];
        thread.messages.push(index);
        thread.unread |= message.unread;
        thread.answered |= message.answered;
        thread.attachments |= !message.attachments.is_empty();
    }

    for (index, message) in elsewhere.iter().enumerate() {
        if let Some(&at) = by_key.get(&Key::of(message)) {
            let thread = &mut threads.as_mut_slice()[at];
            thread.stack.push(Place::Elsewhere(index));
            thread.answered |= message.answered;
            thread.attachments |= !message.attachments.is_empty();
        }
    }

    // Of one date, the shown Folder's go first, its older first, then
    // those elsewhere as given; unstable sorts by it never allocate.
    let order = |place: &Place| match *place {
        Place::Here(i) => (messages[i].received, 0, Reverse(i), 0),
        Place::Elsewhere(i) => (elsewhere[i].received, 1, Reverse(0), i),
    };

    for thread in threads.as_mut_slice() {
        let here = thread.messages.iter().rev().map(|&i| Place::Here(i));
        thread.stack.splice(0..0, here);
        thread.stack.sort_unstable_by_key(order);

        for &index in thread.messages.iter().rev() {
            let name = sender(&messages[index]);
            if !thread.senders.iter().any(|&s| sender(&messages[s]) == name) {
                thread.senders.push(index);
            }
        }
    }
}

/// A Thread `key` with nothing in it yet, in the next slot of `threads`;
/// where it is.
fn open(threads: &mut Pool<Thread>, key: Key) -> usize {
    let thread = threads.next(|| Thread {
        key,
        messages: Vec::new(),
        stack: Vec::new(),
        senders: Vec::new(),
        unread: false,
        answered: false,
        attachments: false,
    });

    thread.key = key;
    thread.messages.clear();
    thread.stack.clear();
    thread.senders.clear();
    thread.unread = false;
    thread.answered = false;
    thread.attachments = false;
    threads.as_slice().len() - 1
}
