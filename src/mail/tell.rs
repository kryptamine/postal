use crate::{mail::arrivals::Arrivals, mail::status::Status};

/// What a sync connection tells the screen: `send` follows every Cache
/// write, `report` the state of the connection, `arrivals` picks the new mail
/// worth a notification.
pub struct Tell<'a, F> {
    pub send: &'a F,
    pub report: &'a dyn Fn(Status),
    pub arrivals: &'a Arrivals,
}

// A derive would ask `F: Copy` of what is only borrowed.
impl<F> Clone for Tell<'_, F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for Tell<'_, F> {}
