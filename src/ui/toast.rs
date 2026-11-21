use std::{borrow::Cow, time::Duration};

use tokio::time::Instant;

const SHOWN_FOR: Duration = Duration::from_secs(3);

/// A short notice in the hint line's right slot. There is no queue: a new
/// Toast replaces the one shown.
pub struct Toast {
    pub text: Cow<'static, str>,
    pub until: Instant,
    /// Drawn in the error colour rather than the default foreground.
    pub error: bool,
}

impl Toast {
    pub fn new(text: impl Into<Cow<'static, str>>) -> Self {
        Self {
            text: text.into(),
            until: Instant::now() + SHOWN_FOR,
            error: false,
        }
    }

    pub fn error(text: impl Into<Cow<'static, str>>) -> Self {
        Self {
            error: true,
            ..Self::new(text)
        }
    }
}
