use futures::channel::mpsc::UnboundedSender;

/// One channel per Account, by its place in this run.
pub struct Routes<T>(Vec<UnboundedSender<T>>);

impl<T> Routes<T> {
    pub const fn new(senders: Vec<UnboundedSender<T>>) -> Self {
        Self(senders)
    }

    /// Sends `item` to `account`; nothing when its work has stopped, as
    /// while it is signed out.
    pub fn send(&self, account: usize, item: T) {
        if let Some(sender) = self.0.get(account) {
            sender.unbounded_send(item).ok();
        }
    }
}

impl<T> Clone for Routes<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
