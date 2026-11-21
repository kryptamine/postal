use futures::channel::mpsc::UnboundedSender;

use crate::domain::per_account::{AccountId, PerAccount};

/// One channel per Account.
pub struct Routes<T>(PerAccount<UnboundedSender<T>>);

impl<T> Routes<T> {
    pub const fn new(senders: PerAccount<UnboundedSender<T>>) -> Self {
        Self(senders)
    }

    /// Sends `item` to `account`; nothing when its work has stopped, as
    /// while it is signed out.
    pub fn send(&self, account: AccountId, item: T) {
        self.0[account].unbounded_send(item).ok();
    }
}

impl<T> Clone for Routes<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
