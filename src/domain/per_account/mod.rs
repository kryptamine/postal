mod account_id;

use std::ops::{Index, IndexMut};

pub use account_id::AccountId;

/// One `T` for each Account of this run, in the config's order. Every
/// `PerAccount` of a run holds as many, and an `AccountId` comes only from
/// iterating one, so an index never falls outside.
#[derive(Clone, Default)]
pub struct PerAccount<T>(Vec<T>);

impl<T> PerAccount<T> {
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    pub fn ids(&self) -> impl DoubleEndedIterator<Item = AccountId> + use<T> {
        (0..self.0.len()).map(AccountId)
    }

    pub fn first(&self) -> Option<AccountId> {
        self.ids().next()
    }

    pub fn iter(&self) -> impl Iterator<Item = (AccountId, &T)> {
        self.0.iter().enumerate().map(|(i, t)| (AccountId(i), t))
    }

    pub fn values(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }

    /// The first Account whose `T` satisfies `matches`.
    pub fn position(&self, matches: impl Fn(&T) -> bool) -> Option<AccountId> {
        self.0.iter().position(matches).map(AccountId)
    }

    pub fn map<'a, U>(&'a self, f: impl FnMut(&'a T) -> U) -> PerAccount<U> {
        PerAccount(self.0.iter().map(f).collect())
    }
}

impl<T> FromIterator<T> for PerAccount<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<T> Index<AccountId> for PerAccount<T> {
    type Output = T;

    #[expect(
        clippy::indexing_slicing,
        reason = "every PerAccount of a run is as long as the one the id came from"
    )]
    fn index(&self, account: AccountId) -> &T {
        &self.0[account.0]
    }
}

impl<T> IndexMut<AccountId> for PerAccount<T> {
    #[expect(
        clippy::indexing_slicing,
        reason = "every PerAccount of a run is as long as the one the id came from"
    )]
    fn index_mut(&mut self, account: AccountId) -> &mut T {
        &mut self.0[account.0]
    }
}
