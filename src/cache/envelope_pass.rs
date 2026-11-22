use rusqlite::{OptionalExtension, params};

use super::{Cache, Error, correspondence};
use crate::{domain::correspondence::Correspondence, domain::uid::Uid};

/// How far the pass over every Message's envelope has read in one Folder.
pub struct EnvelopePass {
    pub folder: String,
    pub uid_validity: u32,
    /// The highest UID read: newer ones are read next.
    pub last_uid: Uid,
    /// Envelopes below this UID are still to read; `None` once none are.
    pub below: Option<Uid>,
}

impl Cache {
    pub fn envelope_pass(&self) -> Result<Option<EnvelopePass>, Error> {
        let pass = self
            .connection()
            .prepare_cached(
                "SELECT mailbox, uid_validity, last_uid, below FROM envelope_pass
                 WHERE account = ?1",
            )?
            .query_row([self.email()], |row| {
                Ok(EnvelopePass {
                    folder: row.get(0)?,
                    uid_validity: row.get(1)?,
                    last_uid: row.get(2)?,
                    below: row.get(3)?,
                })
            })
            .optional()?;
        Ok(pass)
    }

    /// Learns from `read` and keeps how far the pass has read, in one
    /// transaction. A Message from the Account's own address counts as sent.
    pub fn write_envelope_pass(
        &self,
        pass: &EnvelopePass,
        mut read: Vec<(Uid, Correspondence)>,
    ) -> Result<(), Error> {
        let own = |seen: &Correspondence| {
            seen.from
                .iter()
                .any(|p| p.address.eq_ignore_ascii_case(self.email()))
        };

        for (_, seen) in read.iter_mut().filter(|(_, seen)| own(seen)) {
            seen.sent = true;
            seen.from.clear();
        }

        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            for (_, seen) in &read {
                correspondence::record(&transaction, self.email(), seen)?;
            }

            transaction.execute(
                "INSERT OR REPLACE INTO envelope_pass
                 (account, mailbox, uid_validity, last_uid, below)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    self.email(),
                    pass.folder,
                    pass.uid_validity,
                    pass.last_uid,
                    pass.below
                ],
            )?;
            transaction.commit()
        })
    }
}
