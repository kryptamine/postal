use std::ops::{Deref, DerefMut};

use crate::{mail::pending::Pending, mail::update::Update};

/// `Pending` lent to one fetch connection: the one-shots left when it ends
/// fail as offline, also when a wake from sleep drops it mid-await.
pub struct OneshotAttempt<'a, F: Fn(Update)> {
    pub pending: &'a mut Pending,
    pub send: &'a F,
}

impl<F: Fn(Update)> Deref for OneshotAttempt<'_, F> {
    type Target = Pending;

    fn deref(&self) -> &Pending {
        self.pending
    }
}

impl<F: Fn(Update)> DerefMut for OneshotAttempt<'_, F> {
    fn deref_mut(&mut self) -> &mut Pending {
        self.pending
    }
}

impl<F: Fn(Update)> Drop for OneshotAttempt<'_, F> {
    fn drop(&mut self) {
        self.pending.fail_oneshots(self.send);
    }
}
