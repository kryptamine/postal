use std::path::Path;

use super::{Error, schema};

/// The tables of an Account's Folders, Messages and moves, each naming it
/// in `account`.
const TABLES: [&str; 6] = [
    "mailbox",
    "message",
    "thread_message",
    "body",
    "flag_change",
    "moved",
];

/// Deletes every row of the Account `email` from the Cache in `dir`.
pub fn forget(dir: &Path, email: &str) -> Result<(), Error> {
    let (mut connection, _) = schema::open(dir)?;
    let transaction = connection.transaction()?;
    let email = email.to_lowercase();
    for table in TABLES {
        transaction.execute(&format!("DELETE FROM {table} WHERE account = ?1"), [&email])?;
    }

    transaction.commit()?;
    Ok(())
}
