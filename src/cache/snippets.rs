use rusqlite::{Row, params};

use super::{Cache, Error};
use crate::{text_part::TextPart, transfer_encoding::TransferEncoding};

impl Cache {
    /// Up to `limit` Messages of `folder` still without a Snippet, newest
    /// first, and the part each one's Snippet is cut from; only UIDs below
    /// `below` when given.
    pub fn pending_snippets(
        &self,
        folder: &str,
        limit: usize,
        below: Option<u32>,
    ) -> Result<Vec<(u32, TextPart)>, Error> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let connection = self.connection();
        let mut statement = connection.prepare_cached(
            "SELECT uid, text_section, text_encoding, text_charset, text_html FROM message
             WHERE account = ?4 AND mailbox = ?3 AND snippet IS NULL
               AND (?2 IS NULL OR uid < ?2)
             ORDER BY received DESC, uid DESC LIMIT ?1",
        )?;
        // A Message with no text part was written with an empty Snippet.
        let pending = statement
            .query_map(params![limit, below, folder, self.email()], |row| {
                Ok((row.get(0)?, text_part(row, 1)?))
            })?
            .filter_map(|row| row.map(|(uid, part)| part.map(|p| (uid, p))).transpose())
            .collect::<Result<_, _>>()?;
        Ok(pending)
    }

    /// Writes one batch of Snippets in one transaction; a UID no longer
    /// cached is ignored.
    pub fn write_snippets(&self, folder: &str, snippets: &[(u32, String)]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut update = transaction.prepare_cached(
                "UPDATE message SET snippet = ?2 WHERE account = ?4 AND mailbox = ?3 AND uid = ?1",
            )?;
            for (uid, snippet) in snippets {
                update.execute(params![uid, snippet, folder, self.email()])?;
            }

            drop(update);
            transaction.commit()
        })
    }
}

/// The text part in the four columns from `first`: section, encoding,
/// charset and html; `None` when the section is NULL.
fn text_part(row: &Row<'_>, first: usize) -> rusqlite::Result<Option<TextPart>> {
    let Some(section) = row.get::<_, Option<String>>(first)? else {
        return Ok(None);
    };

    let encoding: String = row.get(first + 1)?;
    Ok(Some(TextPart {
        section,
        encoding: TransferEncoding::from_label(&encoding),
        charset: row.get(first + 2)?,
        html: row.get(first + 3)?,
    }))
}
