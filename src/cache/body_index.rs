use rusqlite::params;

use super::{Cache, Error, snippets::text_part};
use crate::{domain::folder_name::FolderName, domain::text_part::TextPart, domain::uid::Uid};

/// A Message whose body the index lacks.
pub struct Unindexed {
    pub uid: Uid,
    /// `None` with no text part: there is nothing to fetch.
    pub part: Option<TextPart>,
    pub snippet_missing: bool,
}

/// What one Message's body gives the index.
pub struct Indexed {
    pub uid: Uid,
    pub text: Option<String>,
    /// Written only to a row still without one.
    pub snippet: Option<String>,
}

impl Cache {
    /// The Folder of the newest Message whose body the index lacks, outside
    /// Trash and Spam, and up to `limit` of its such Messages, newest first.
    pub fn unindexed_bodies(
        &self,
        limit: usize,
    ) -> Result<Option<(FolderName, Vec<Unindexed>)>, Error> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let connection = self.connection();
        let folder: Option<FolderName> = connection
            .prepare_cached(
                "SELECT m.mailbox FROM message m INDEXED BY message_unbodied
                 WHERE m.account = ?1 AND m.bodied = 0 AND NOT EXISTS
                   (SELECT 1 FROM mailbox b WHERE b.account = m.account AND b.name = m.mailbox
                    AND b.role IN ('Trash', 'Spam'))
                 ORDER BY m.rowid DESC LIMIT 1",
            )?
            .query_map(params![self.email()], |row| row.get(0))?
            .next()
            .transpose()?;
        let Some(folder) = folder else {
            return Ok(None);
        };

        let mut statement = connection.prepare_cached(
            "SELECT uid, text_section, text_encoding, text_charset, text_html, snippet IS NULL
             FROM message INDEXED BY message_unbodied
             WHERE account = ?1 AND bodied = 0 AND mailbox = ?2
             ORDER BY rowid DESC LIMIT ?3",
        )?;
        let pending = statement
            .query_map(params![self.email(), folder, limit], |row| {
                Ok(Unindexed {
                    uid: row.get(0)?,
                    part: text_part(row, 1)?,
                    snippet_missing: row.get(5)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        drop(statement);
        Ok(Some((folder, pending)))
    }

    /// Indexes each body in `folder`, in every Folder holding that Message,
    /// in one transaction; a UID no longer cached is ignored.
    pub fn index_bodies(&self, folder: &FolderName, bodies: &[Indexed]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            // Rewriting the Snippet indexes the row anew, so it goes first.
            let mut snippet = transaction.prepare_cached(
                "UPDATE message SET snippet = ?4
                 WHERE account = ?1 AND mailbox = ?2 AND uid = ?3 AND snippet IS NULL",
            )?;
            let mut forget = transaction.prepare_cached(
                "DELETE FROM message_text WHERE rowid IN (SELECT rowid FROM message
                 WHERE account = ?1 AND body_key = (SELECT body_key FROM message
                 WHERE account = ?1 AND mailbox = ?2 AND uid = ?3))",
            )?;
            let mut index = transaction.prepare_cached(
                "INSERT INTO message_text
                     (rowid, sender, subject, snippet, recipients, from_line, to_line, body)
                 SELECT rowid, sender, subject, snippet, iif(from_me, to_line, NULL),
                     from_line, to_line, ?4
                 FROM message WHERE account = ?1 AND body_key = (SELECT body_key FROM message
                 WHERE account = ?1 AND mailbox = ?2 AND uid = ?3)",
            )?;
            let mut mark = transaction.prepare_cached(
                "UPDATE message SET bodied = 1 WHERE account = ?1 AND body_key =
                 (SELECT body_key FROM message WHERE account = ?1 AND mailbox = ?2 AND uid = ?3)",
            )?;
            for body in bodies {
                let row = params![self.email(), folder, body.uid];
                if let Some(text) = &body.snippet {
                    snippet.execute(params![self.email(), folder, body.uid, text])?;
                }

                forget.execute(row)?;
                index.execute(params![self.email(), folder, body.uid, body.text])?;
                mark.execute(row)?;
            }

            drop((snippet, forget, index, mark));
            transaction.commit()
        })
    }
}
