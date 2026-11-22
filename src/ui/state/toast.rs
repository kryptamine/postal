use std::{borrow::Cow, time::Duration};

use tokio::time::Instant;

const SHOWN_FOR: Duration = Duration::from_secs(3);
/// How long a lasting Toast stays if nothing replaces it.
const LASTING: Duration = Duration::from_secs(600);

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

    /// Shown until another replaces it, as while something is on its way.
    pub fn lasting(text: impl Into<Cow<'static, str>>) -> Self {
        Self {
            until: Instant::now() + LASTING,
            ..Self::new(text)
        }
    }

    pub fn error(text: impl Into<Cow<'static, str>>) -> Self {
        Self {
            error: true,
            ..Self::new(text)
        }
    }
}
