use tokio::sync::Notify;

static ASKED: Notify = Notify::const_new();

/// Cuts short every backoff under way: each Account offline reconnects now.
/// One asked while a reconnect is under way is not kept for the next wait.
pub fn ask() {
    ASKED.notify_waiters();
}

/// Resolves on the next `ask`.
pub async fn asked() {
    ASKED.notified().await;
}
