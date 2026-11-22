use rusqlite::{Connection, OptionalExtension, params};

use super::{Cache, Error, folder_key::FolderKey};
use crate::{
    domain::correspondence::Correspondence, domain::folder::Role, domain::new_message::NewMessage,
    domain::uid::Uid,
};

/// How far the pass over the Folder with the Sent role has read.
pub struct SentPass {
    pub folder: String,
    pub uid_validity: u32,
    pub last_uid: Uid,
}

impl Cache {
    /// Learns from a Message the Outbox sent.
    pub fn sent(&self, sent: &Correspondence) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            record(&transaction, self.email(), sent)?;
            transaction.commit()
        })
    }

    pub fn sent_pass(&self) -> Result<Option<SentPass>, Error> {
        let pass = self
            .connection()
            .prepare_cached(
                "SELECT mailbox, uid_validity, last_uid FROM sent_pass WHERE account = ?1",
            )?
            .query_row([self.email()], |row| {
                Ok(SentPass {
                    folder: row.get(0)?,
                    uid_validity: row.get(1)?,
                    last_uid: row.get(2)?,
                })
            })
            .optional()?;
        Ok(pass)
    }

    /// Learns from `read`, the Messages of the Sent pass, and keeps how far
    /// it read, in one transaction.
    pub fn write_sent_pass(
        &self,
        pass: &SentPass,
        read: &[(Uid, Correspondence)],
    ) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            for (_, correspondence) in read {
                record(&transaction, self.email(), correspondence)?;
            }

            transaction.execute(
                "INSERT OR REPLACE INTO sent_pass (account, mailbox, uid_validity, last_uid)
                 VALUES (?1, ?2, ?3, ?4)",
                params![self.email(), pass.folder, pass.uid_validity, pass.last_uid],
            )?;
            transaction.commit()
        })
    }
}

/// Learns from `rows` fetched into `folder`, unless it is Trash or Spam.
pub(super) fn learn(
    connection: &Connection,
    key: FolderKey<'_>,
    rows: &[NewMessage],
) -> rusqlite::Result<()> {
    let FolderKey { account, folder } = key;
    let role: Option<String> = connection
        .prepare_cached("SELECT role FROM mailbox WHERE account = ?1 AND name = ?2")?
        .query_row(params![account, folder], |row| row.get(0))
        .optional()?
        .flatten();
    let shunned = [Role::Trash, Role::Spam].map(Role::label);
    if role.is_some_and(|role| shunned.contains(&role.as_str())) {
        return Ok(());
    }

    for new in rows {
        record(connection, account, &new.correspondence)?;
    }

    Ok(())
}

pub(super) fn record(
    connection: &Connection,
    account: &str,
    seen: &Correspondence,
) -> rusqlite::Result<()> {
    let mut insert = connection.prepare_cached(
        "INSERT INTO correspondence (account, address, message, name, written, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT DO UPDATE SET written = written OR excluded.written",
    )?;
    let from = seen.from.iter().map(|person| (person, false));
    let to = seen.to.iter().map(|person| (person, seen.sent));
    for (person, written) in from.chain(to) {
        insert.execute(params![
            account,
            person.address,
            seen.message,
            person.name,
            written,
            seen.at.timestamp()
        ])?;
    }

    Ok(())
}
