/// An Account by its place in this run, made only by iterating a
/// `PerAccount`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AccountId(pub(super) usize);

impl AccountId {
    /// Its place in the config, for what lists Accounts in that order.
    pub const fn place(self) -> usize {
        self.0
    }
}
