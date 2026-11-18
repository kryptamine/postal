use std::time::Duration;

use tokio::time::Instant;

const SHOWN_FOR: Duration = Duration::from_secs(3);

/// A short notice in the hint line's right slot. There is no queue: a new
/// Toast replaces the one shown.
pub struct Toast {
    pub text: &'static str,
    pub until: Instant,
}

impl Toast {
    pub fn new(text: &'static str) -> Self {
        Self {
            text,
            until: Instant::now() + SHOWN_FOR,
        }
    }
}
