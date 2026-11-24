use rusqlite::params;

use super::{Cache, Error, snippets::text_part};
use crate::{
    domain::body_key::BodyKey, domain::folder_name::FolderName, domain::indexing::Indexing,
    domain::text_part::TextPart, domain::uid::Uid,
};

/// A Message whose body the index lacks.
pub struct Unindexed {
    pub uid: Uid,
    pub key: BodyKey,
    /// `None` with no text part: there is nothing to fetch.
    pub part: Option<TextPart>,
    pub snippet_missing: bool,
}

/// What one Message's body gives the index.
pub struct Indexed {
    pub uid: Uid,
    /// The row's key when it was read: a row since replaced is left alone.
    pub key: BodyKey,
    pub text: Option<String>,
    /// Written only to a row still without one.
    pub snippet: Option<String>,
}

impl Cache {
    /// The Folder of the newest Message whose body the index lacks, and up
    /// to `limit` of its such Messages, newest first.
    pub fn unindexed_bodies(
        &self,
        limit: usize,
    ) -> Result<Option<(FolderName, Vec<Unindexed>)>, Error> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let connection = self.connection();
        let folder: Option<FolderName> = connection
            .prepare_cached(
                "SELECT mailbox FROM message INDEXED BY message_unbodied
                 WHERE account = ?1 AND bodied = 0 ORDER BY rowid DESC LIMIT 1",
            )?
            .query_map(params![self.email()], |row| row.get(0))?
            .next()
            .transpose()?;
        let Some(folder) = folder else {
            return Ok(None);
        };

        let mut statement = connection.prepare_cached(
            "SELECT uid, text_section, text_encoding, text_charset, text_html, snippet IS NULL,
                 body_key
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
                    key: row.get(6)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        drop(statement);
        Ok(Some((folder, pending)))
    }

    /// How far the index is, counting as not cached yet what the server
    /// last counted in `walked` past the rows cached there. 22 ms at
    /// 500,000: the partial index, then every row's Folder once.
    pub fn indexing(&self, walked: &[FolderName]) -> Result<Indexing, Error> {
        let connection = self.connection();
        let unbodied: usize = connection
            .prepare_cached(
                "SELECT count(*) FROM message INDEXED BY message_unbodied
                 WHERE account = ?1 AND bodied = 0",
            )?
            .query_row(params![self.email()], |row| row.get(0))?;
        let mut statement = connection.prepare_cached(
            "SELECT m.mailbox, count(*), b.exists_count FROM message m
             LEFT JOIN mailbox b ON b.account = m.account AND b.name = m.mailbox
             WHERE m.account = ?1 GROUP BY m.mailbox",
        )?;
        let (mut cached, mut missing) = (0, 0);
        let folders = statement.query_map(params![self.email()], |row| {
            Ok((
                row.get::<_, FolderName>(0)?,
                row.get::<_, usize>(1)?,
                row.get::<_, Option<usize>>(2)?,
            ))
        })?;
        for folder in folders {
            let (name, rows, exists) = folder?;
            cached += rows;
            if walked.contains(&name) {
                missing += exists.unwrap_or(0).saturating_sub(rows);
            }
        }

        Ok(Indexing {
            indexed: cached - unbodied.min(cached),
            total: cached + missing,
        })
    }

    /// Indexes each body in `folder` in one transaction, on Gmail in every
    /// Folder holding that Message; a row no longer cached as it was read
    /// is left alone.
    pub fn index_bodies(&self, folder: &FolderName, bodies: &[Indexed]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            // Elsewhere a body key repeats across Folders that share a
            // UIDVALIDITY, so only Gmail's, `X-GM-MSGID`, names copies.
            let mut copies = transaction.prepare_cached(
                "SELECT rowid FROM message WHERE account = ?1 AND body_key = ?4
                   AND (thread IS NOT NULL OR (mailbox = ?2 AND uid = ?3))
                   AND EXISTS (SELECT 1 FROM message WHERE account = ?1 AND mailbox = ?2
                               AND uid = ?3 AND body_key = ?4)",
            )?;
            // Rewriting the Snippet indexes the row anew, so it goes first.
            let mut snippet = transaction.prepare_cached(
                "UPDATE message SET snippet = ?2 WHERE rowid = ?1 AND snippet IS NULL",
            )?;
            let mut forget =
                transaction.prepare_cached("DELETE FROM message_text WHERE rowid = ?1")?;
            // The columns the triggers of migrations/4.sql index, and the body.
            let mut index = transaction.prepare_cached(
                "INSERT INTO message_text
                     (rowid, sender, subject, snippet, recipients, from_line, to_line, body)
                 SELECT rowid, sender, subject, snippet, iif(from_me, to_line, NULL),
                     from_line, to_line, ?2
                 FROM message WHERE rowid = ?1",
            )?;
            let mut mark =
                transaction.prepare_cached("UPDATE message SET bodied = 1 WHERE rowid = ?1")?;
            for body in bodies {
                let rowids: Vec<i64> = copies
                    .query_map(params![self.email(), folder, body.uid, body.key], |row| {
                        row.get(0)
                    })?
                    .collect::<Result<_, _>>()?;
                for rowid in rowids {
                    if let Some(text) = &body.snippet {
                        snippet.execute(params![rowid, text])?;
                    }

                    forget.execute([rowid])?;
                    index.execute(params![rowid, body.text])?;
                    mark.execute([rowid])?;
                }
            }

            drop((copies, snippet, forget, index, mark));
            transaction.commit()
        })
    }
}
