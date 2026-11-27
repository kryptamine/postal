use std::time::Duration;

use tokio::time::Instant;

use crate::{domain::body_key::BodyKey, domain::thread::Key};

/// How long the selection rests on a Thread before a Mark Read: long enough
/// to scroll past a row without marking it.
const DELAY: Duration = Duration::from_secs(1);

/// The selection resting on an Unread Thread whose Preview shows its body.
pub struct Dwell {
    /// The Thread, the body key of its newest Message, and since when.
    on: Option<(Key, BodyKey, Instant)>,
    /// Marked Unread by hand: no Dwell on it until the selection leaves it.
    spared: Option<Key>,
    delay: Duration,
}

impl Default for Dwell {
    /// Tests set `POSTAL_DWELL` in seconds, a fraction too: a test that
    /// checks no Mark Read comes waits out a whole Dwell.
    fn default() -> Self {
        let set = std::env::var("POSTAL_DWELL").ok();
        let test = set.and_then(|secs| Duration::try_from_secs_f64(secs.parse().ok()?).ok());
        Self {
            on: None,
            spared: None,
            delay: test.unwrap_or(DELAY),
        }
    }
}

impl Dwell {
    /// Follows the screen: `selected` is the selected Thread, `target` it
    /// with its newest body key while it is Unread and its body shows. A
    /// new newest Message starts the Dwell over.
    pub fn follow(&mut self, selected: Option<Key>, target: Option<(Key, BodyKey)>, now: Instant) {
        if self.spared != selected {
            self.spared = None;
        }

        let target = target.filter(|&(key, _)| self.spared != Some(key));
        let same = matches!((self.on, target), (Some((k, b, _)), Some(t)) if (k, b) == t);
        if !same {
            self.on = target.map(|(key, body)| (key, body, now));
        }
    }

    /// When the Dwell under way ends.
    pub fn due(&self) -> Option<Instant> {
        self.on.map(|(_, _, since)| since + self.delay)
    }

    /// Whether the Dwell under way has ended by `now`; it is over then.
    pub fn ended(&mut self, now: Instant) -> bool {
        let ended = self.due().is_some_and(|due| due <= now);
        if ended {
            self.on = None;
        }

        ended
    }

    /// No Dwell on `key` until the selection leaves it.
    pub const fn spare(&mut self, key: Key) {
        self.spared = Some(key);
        self.on = None;
    }
}
