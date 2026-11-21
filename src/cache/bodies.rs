use rusqlite::{Connection, OptionalExtension, params};

use super::{Cache, Error};

use crate::body_key::BodyKey;

/// The raw bodies kept, in bytes; past it the least recently used go first.
const BODY_BYTES: i64 = 200 * 1024 * 1024;

impl Cache {
    pub fn has_body(&self, key: BodyKey) -> Result<bool, Error> {
        let found = self
            .connection()
            .prepare_cached("SELECT 1 FROM body WHERE account = ?2 AND key = ?1")?
            .query_row(params![key, self.email()], |_| Ok(()))
            .optional()?;
        Ok(found.is_some())
    }

    /// The body cached under `key`, marked as just used; `None` until it is
    /// fetched.
    pub fn body(&self, key: BodyKey) -> Result<Option<Vec<u8>>, Error> {
        let found = self
            .connection()
            .prepare_cached("SELECT raw FROM body WHERE account = ?2 AND key = ?1")?
            .query_row(params![key, self.email()], |row| row.get(0))
            .optional()?;
        let Some(body) = found else {
            return Ok(None);
        };

        self.write(|connection| {
            connection
                .prepare_cached(
                    "UPDATE body SET used = (SELECT max(used) + 1 FROM body)
                     WHERE account = ?2 AND key = ?1",
                )?
                .execute(params![key, self.email()])
                .map(drop)
        })?;
        Ok(Some(body))
    }

    /// Stores each `(uid, raw)` under its Message's body key in one
    /// transaction, then evicts past `BODY_BYTES`. A UID no longer cached is
    /// ignored.
    pub fn write_bodies(&self, folder: &str, bodies: &[(u32, Vec<u8>)]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut insert = transaction.prepare_cached(
                "INSERT OR REPLACE INTO body (account, key, raw, size, used)
                 SELECT account, body_key, ?2, ?3, (SELECT coalesce(max(used), 0) + 1 FROM body)
                 FROM message WHERE account = ?5 AND mailbox = ?4 AND uid = ?1",
            )?;
            for (uid, raw) in bodies {
                insert.execute(params![uid, raw, raw.len(), folder, self.email()])?;
            }

            drop(insert);
            evict(&transaction)?;
            transaction.commit()
        })
    }
}

pub(super) fn store(
    connection: &Connection,
    account: &str,
    bodies: &[(BodyKey, Vec<u8>)],
) -> rusqlite::Result<()> {
    let mut insert = connection.prepare_cached(
        "INSERT OR REPLACE INTO body (account, key, raw, size, used)
         VALUES (?4, ?1, ?2, ?3, (SELECT coalesce(max(used), 0) + 1 FROM body))",
    )?;
    for (key, raw) in bodies {
        insert.execute(params![key, raw, raw.len(), account])?;
    }

    Ok(())
}

/// Drops the least recently used bodies past `BODY_BYTES`, of every Account.
pub(super) fn evict(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM body WHERE rowid IN (
             SELECT rowid FROM (
                 SELECT rowid, sum(size) OVER (ORDER BY used DESC) AS kept FROM body
             ) WHERE kept > ?1
         )",
        [BODY_BYTES],
    )?;
    Ok(())
}
