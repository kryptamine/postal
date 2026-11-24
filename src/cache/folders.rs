use rusqlite::{Connection, params};

use super::{Cache, Error, folder_key::FolderKey};
use crate::domain::counts::Counts;
use crate::domain::folder::{Folder, Role};
use crate::domain::folder_name::FolderName;

impl Cache {
    /// Every Folder listed or shown so far, in the Folder Picker's order.
    pub fn folders(&self) -> Result<Vec<Folder>, Error> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(
            "SELECT name, role, unread, messages FROM mailbox WHERE account = ?1",
        )?;
        let mut folders = statement
            .query_map([self.email()], |row| {
                let role: Option<String> = row.get(1)?;
                let folder = Folder::new(
                    self.account(),
                    &row.get::<_, String>(0)?,
                    role.and_then(|r| Role::from_label(&r)),
                );
                Ok(Folder {
                    counts: counts(row.get(2)?, row.get(3)?),
                    ..folder
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        folders.sort_by(Folder::order);
        Ok(folders)
    }

    /// The Folder named `name` as last listed; without a role when it never was.
    pub fn folder(&self, name: &FolderName) -> Result<Folder, Error> {
        let listed = self.folders()?.into_iter().find(|f| f.name == *name);
        Ok(listed.unwrap_or_else(|| Folder::new(self.account(), name.as_str(), None)))
    }

    /// Writes what `LIST` answered: a Folder not in `listed` leaves the
    /// Cache with its rows, and the rest keep what they had.
    pub fn list_folders(&self, listed: &[Folder<()>]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let cached: Vec<FolderName> = transaction
                .prepare_cached("SELECT name FROM mailbox WHERE account = ?1")?
                .query_map([self.email()], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            let mut upsert = transaction.prepare_cached(
                "INSERT INTO mailbox (account, name, role) VALUES (?3, ?1, ?2)
                 ON CONFLICT (account, name) DO UPDATE SET role = excluded.role",
            )?;
            for folder in listed {
                upsert.execute(params![
                    &folder.name,
                    folder.role.map(Role::label),
                    self.email()
                ])?;
            }

            for gone in cached
                .iter()
                .filter(|c| !listed.iter().any(|f| f.name == **c))
            {
                forget(&transaction, self.key(gone))?;
            }

            drop(upsert);
            transaction.commit()
        })
    }

    /// The counts `STATUS` gave for `folder`; one no longer listed is ignored.
    pub fn write_counts(&self, folder: &FolderName, counts: Counts) -> Result<(), Error> {
        self.write(|connection| {
            connection
                .prepare_cached(
                    "UPDATE mailbox SET unread = ?2, messages = ?3 WHERE account = ?4 AND name = ?1",
                )?
                .execute(params![folder, counts.unread, counts.total, self.email()])
                .map(drop)
        })
    }

    /// Drops `folder` and its rows, as for a Folder the server no longer has.
    pub fn remove_folder(&self, folder: &FolderName) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            forget(&transaction, self.key(folder))?;
            transaction.commit()
        })
    }
}

/// Drops `folder` with its rows and its queued changes.
fn forget(connection: &Connection, key: FolderKey<'_>) -> rusqlite::Result<()> {
    let FolderKey { account, folder } = key;
    for table in ["message", "flag_change", "moved"] {
        connection.execute(
            &format!("DELETE FROM {table} WHERE account = ?2 AND mailbox = ?1"),
            params![folder, account],
        )?;
    }

    connection.execute(
        "DELETE FROM mailbox WHERE account = ?2 AND name = ?1",
        params![folder, account],
    )?;
    Ok(())
}

/// A Cache from before the total was kept has the Unread count alone until
/// the next `STATUS`: that row counts as not counted yet.
pub(super) fn counts(unread: Option<usize>, total: Option<usize>) -> Option<Counts> {
    Some(Counts {
        unread: unread?,
        total: total?,
    })
}
