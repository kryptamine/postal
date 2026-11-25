use std::time::Duration;

use tokio::time::Instant;

use crate::mail::held;

/// The longest a Send under way holds up a move to another terminal.
const SEND_WAIT: Duration = Duration::from_secs(15);

/// When the event loop ends for a TUI in another terminal: as soon as it
/// is asked and no Send is under way, or once one has had `SEND_WAIT`.
pub struct Leaving {
    deadline: Option<Instant>,
    _serving: held::Serving,
}

impl Leaving {
    pub fn start() -> Self {
        Self {
            deadline: None,
            _serving: held::Serving::start(),
        }
    }

    pub fn due(&mut self) -> bool {
        if held::released() {
            return true;
        }

        if !held::leaving() {
            return false;
        }

        let deadline = *self
            .deadline
            .get_or_insert_with(|| Instant::now() + SEND_WAIT);
        !held::sending() || Instant::now() >= deadline
    }

    /// Wakes when `due` may have changed.
    pub async fn woken(&self) {
        match self.deadline {
            Some(deadline) => {
                tokio::select! {
                    () = held::woken() => {}
                    () = tokio::time::sleep_until(deadline) => {}
                }
            }
            None => held::woken().await,
        }
    }
}
