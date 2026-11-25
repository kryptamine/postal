use std::{
    thread::{self, JoinHandle},
    time::Duration,
};

use tokio::time::Instant;

use crate::{account::config::ContactsChoice, account::contacts, ui::state::toast::Toast};

/// How often a read under way is looked at.
const POLL: Duration = Duration::from_millis(200);

/// The read of Contacts that turning them on starts, so macOS asks for
/// access then rather than in the middle of a completion.
#[derive(Default)]
pub struct ContactsCheck(Option<JoinHandle<Option<String>>>);

impl ContactsCheck {
    pub fn start(&mut self, choice: ContactsChoice) {
        self.0 = Some(thread::spawn(move || {
            let read = contacts::read(choice);
            let failure = read.err().map(|error| error.to_string());
            if let Some(failure) = &failure {
                tracing::warn!("{failure}");
            }

            failure
        }));
    }

    /// When to look at the read again, while it is under way.
    pub fn due(&self, now: Instant) -> Option<Instant> {
        self.0.as_ref().map(|_| now + POLL)
    }

    /// The Toast of a read that ended in a failure.
    pub fn finished(&mut self) -> Option<Toast> {
        if !self.0.as_ref().is_some_and(JoinHandle::is_finished) {
            return None;
        }

        let failure = self.0.take()?.join().ok().flatten();
        failure.map(Toast::error)
    }
}
