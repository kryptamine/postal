use std::{
    hash::{BuildHasher, RandomState},
    time::Duration,
};

const FIRST: Duration = Duration::from_secs(1);
const LAST: Duration = Duration::from_secs(60);

/// The wait before each reconnect: 1, 2, 4 … 60 s, each cut by up to a
/// quarter at random, so clients cut off together do not return together.
pub struct Backoff {
    next: Duration,
}

impl Default for Backoff {
    fn default() -> Self {
        Self { next: FIRST }
    }
}

impl Backoff {
    /// The wait before the next attempt; there is no limit on attempts.
    pub fn next(&mut self) -> Duration {
        let base = self.next;
        self.next = (base * 2).min(LAST);
        let quarter = u64::try_from(base.as_millis() / 4).unwrap_or(u64::MAX);
        // A fresh `RandomState` is seeded at random: enough for jitter.
        let jitter = RandomState::new().hash_one(()) % (quarter + 1);
        base.saturating_sub(Duration::from_millis(jitter))
    }
}
