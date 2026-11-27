use std::time::{Duration, Instant};

use crate::domain::folder_name::FolderName;

/// How often `STATUS` asks after the Inbox while IDLE watches another
/// Folder (ADR 0025).
const EVERY: Duration = Duration::from_secs(2 * 60);

/// When the Inbox is next due a `STATUS`, while another Folder is shown.
pub struct InboxPoll {
    last: Instant,
    every: Duration,
}

impl InboxPoll {
    /// Tests set `POSTAL_INBOX_POLL` in seconds: two minutes is too slow
    /// to wait for.
    pub fn new() -> Self {
        let every = std::env::var("POSTAL_INBOX_POLL")
            .ok()
            .and_then(|secs| secs.parse().ok())
            .map_or(EVERY, Duration::from_secs);
        Self {
            last: Instant::now(),
            every,
        }
    }

    /// How long IDLE on `shown` may last, at most `idle`, before the Inbox
    /// is due; `None` when it is due now, starting the wait for the next
    /// time. Showing the Inbox starts it too: IDLE watches it then.
    pub fn wait(&mut self, shown: &FolderName, idle: Duration) -> Option<Duration> {
        if *shown == *FolderName::inbox() {
            self.last = Instant::now();
            return Some(idle);
        }

        if self.last.elapsed() >= self.every {
            self.last = Instant::now();
            return None;
        }

        Some(idle.min(self.every.saturating_sub(self.last.elapsed())))
    }
}
