use std::{borrow::Cow, fmt::Write, time::Duration};

use tokio::time::Instant;

use crate::{mail::status::seconds_left, render::stack_str::StackStr};

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
    /// A Send held until then, which the text counts down to.
    held: bool,
}

impl Toast {
    pub fn new(text: impl Into<Cow<'static, str>>) -> Self {
        Self {
            text: text.into(),
            until: Instant::now() + SHOWN_FOR,
            error: false,
            held: false,
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

    /// A Send held until `until`, counted down while `u` takes it back.
    pub fn held(until: Instant) -> Self {
        Self {
            until,
            held: true,
            ..Self::new("")
        }
    }

    /// The countdown of a held Send at `now`; `None` for any other Toast.
    pub fn countdown(&self, now: Instant) -> Option<StackStr<48>> {
        if !self.held {
            return None;
        }

        let left = seconds_left(self.until, now);
        let mut text = StackStr::new();
        // Any u64 fits, so the write cannot fail.
        write!(text, "sent in {left}s · u to undo").ok();
        Some(text)
    }

    /// When the countdown next shows a second less.
    pub fn ticks_at(&self, now: Instant) -> Option<Instant> {
        let left = seconds_left(self.until, now);
        (self.held && left > 1).then(|| self.until - Duration::from_secs(left - 1))
    }
}
