use std::{fmt::Write, time::Duration};

use tokio::time::Instant;

use crate::{outage::Outage, stack_str::StackStr};

/// The connection state in the hint line's right slot; silent when live.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Status {
    #[default]
    Live,
    /// The first fetch or a Backfill: `done` of `total` Messages so far.
    Syncing {
        done: u32,
        total: u32,
    },
    Connecting,
    /// Down for `outage`, the next attempt at `retry`.
    Retrying {
        outage: Outage,
        retry: Instant,
    },
    /// The server no longer accepts the Account; nothing is retried.
    SignedOut,
}

impl Status {
    /// The text at `now`, empty when live.
    pub fn text(self, now: Instant) -> StackStr<40> {
        let mut text = StackStr::new();
        // Every text fits, so the write cannot fail.
        let _ = match self {
            Self::Live => Ok(()),
            Self::Syncing { done, total } => write!(text, "syncing {done}/{total}"),
            Self::Connecting => text.write_str("connecting…"),
            Self::Retrying { outage, retry } => {
                let label = match outage {
                    Outage::Offline => "offline",
                    Outage::Busy => "server busy",
                };

                write!(text, "{label} · retry in {}s", seconds_left(retry, now))
            }
            Self::SignedOut => text.write_str("signed out · L sign in"),
        };

        text
    }

    /// Whether the text warns, rather than tells of work under way.
    pub const fn warns(self) -> bool {
        matches!(self, Self::Retrying { .. })
    }

    /// When the countdown next ticks down; `None` while there is none.
    pub fn ticks_at(self, now: Instant) -> Option<Instant> {
        let Self::Retrying { retry, .. } = self else {
            return None;
        };

        let left = seconds_left(retry, now);
        (left > 0).then(|| retry - Duration::from_secs(left - 1))
    }
}

/// Whole seconds until `retry`, rounded up.
fn seconds_left(retry: Instant, now: Instant) -> u64 {
    let left = retry.saturating_duration_since(now);
    left.as_secs() + u64::from(left.subsec_nanos() > 0)
}
