use std::{
    cell::{Cell, Ref, RefCell},
    path::Path,
    rc::Rc,
    sync::Arc,
};

use rusqlite::{
    Connection,
    backup::{Backup, StepResult},
};

use super::{Error, moves, schema};
use crate::domain::per_account::{AccountId, PerAccount};

/// One Account's rows of the Cache, which every Account shares; each row
/// names its Account by address.
#[derive(Clone)]
pub struct Cache {
    shared: Rc<Shared>,
    account: AccountId,
    email: Arc<str>,
}

/// Once a write to disk fails, the Cache moves into memory for the rest of
/// the session, so a full disk or a lost permission stops nothing.
struct Shared {
    connection: RefCell<Connection>,
    rebuilt: bool,
    in_memory: Cell<bool>,
    unreported_failure: Cell<bool>,
}

impl Cache {
    /// The Cache in `dir`, one handle per address of `accounts`, in order.
    pub fn open(dir: &Path, accounts: &PerAccount<&str>) -> Result<PerAccount<Self>, Error> {
        let (connection, rebuilt) = schema::open(dir)?;
        moves::forget_settled(&connection)?;
        let shared = Rc::new(Shared {
            connection: RefCell::new(connection),
            rebuilt,
            in_memory: Cell::new(false),
            unreported_failure: Cell::new(false),
        });

        Ok(accounts
            .iter()
            .map(|(account, email)| Self {
                shared: Rc::clone(&shared),
                account,
                email: email.to_lowercase().into(),
            })
            .collect())
    }

    /// Whether opening replaced a corrupt or outdated Cache.
    pub fn rebuilt(&self) -> bool {
        self.shared.rebuilt
    }

    /// Whether a write to disk failed since the last call. True at most
    /// once a session, as the Cache stays in memory from then on.
    pub fn take_write_failure(&self) -> bool {
        self.shared.unreported_failure.replace(false)
    }

    pub(super) fn connection(&self) -> Ref<'_, Connection> {
        self.shared.connection.borrow()
    }

    pub(super) const fn account(&self) -> AccountId {
        self.account
    }

    pub(super) fn email(&self) -> &str {
        &self.email
    }

    /// Runs `write` on disk; when that fails, copies what the disk holds
    /// into memory and runs it there. `write` must leave nothing behind
    /// when it fails, as a transaction does.
    pub(super) fn write<T>(
        &self,
        write: impl Fn(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, Error> {
        let shared = &self.shared;
        let error = match write(&shared.connection.borrow()) {
            Ok(done) => return Ok(done),
            Err(error) if shared.in_memory.get() => return Err(error.into()),
            Err(error) => error,
        };

        tracing::error!("cache write failed, keeping the Cache in memory: {error}");
        let mut memory = Connection::open_in_memory()?;
        let copied = Backup::new(&shared.connection.borrow(), &mut memory)?.step(-1)?;
        if copied != StepResult::Done {
            return Err(error.into());
        }

        *shared.connection.borrow_mut() = memory;
        shared.in_memory.set(true);
        shared.unreported_failure.set(true);
        Ok(write(&shared.connection.borrow())?)
    }
}
