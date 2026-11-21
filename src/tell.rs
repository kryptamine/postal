use crate::status::Status;

/// What a sync connection tells the screen: `send` follows every Cache
/// write, `report` the state of the connection.
pub struct Tell<'a, F> {
    pub send: &'a F,
    pub report: &'a dyn Fn(Status),
}

// A derive would ask `F: Copy` of what is only borrowed.
impl<F> Clone for Tell<'_, F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for Tell<'_, F> {}
