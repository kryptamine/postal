use std::ops::ControlFlow;

use chrono::{DateTime, FixedOffset};
use rusqlite::{Row, params};

use super::{Error, Reader, received::received, search};
use crate::{
    domain::body_key::BodyKey, domain::folder::Role, domain::root_id::RootId,
    domain::thread_id::ThreadId,
};

/// One Message as a row of a listing, without its body.
pub struct Listed {
    /// The Account's address, lowercased.
    pub account: String,
    pub folder: String,
    pub body_key: BodyKey,
    pub thread: Option<ThreadId>,
    pub root: Option<RootId>,
    pub received: DateTime<FixedOffset>,
    pub unread: bool,
    /// As `Message::from` holds it.
    pub from: String,
    pub subject: String,
    /// Empty until the second fetch phase writes it.
    pub snippet: String,
}

const COLUMNS: &str = "m.account, m.mailbox, m.body_key, m.thread, m.root, m.received,
    m.received_offset, m.unread, m.from_line, m.subject, coalesce(m.snippet, '')";

impl Reader {
    /// The Messages of the Folders `places` names, each by its Account and
    /// name, newest first, until `visit` breaks.
    pub fn listed(
        &self,
        places: &[(&str, &str)],
        mut visit: impl FnMut(Listed) -> ControlFlow<()>,
    ) -> Result<(), Error> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM message m
             WHERE (m.account, m.mailbox) IN
                   (SELECT value ->> 0, value ->> 1 FROM json_each(?1))
             ORDER BY m.received DESC, m.uid DESC"
        ))?;
        let places = serde_json::to_string(places).unwrap_or_default();
        let mut rows = statement.query(params![places])?;
        while let Some(row) = rows.next()? {
            if visit(listed(row)?).is_break() {
                break;
            }
        }

        Ok(())
    }

    /// The Messages of every Folder but Trash and Spam whose words start
    /// with each word of `query`, newest first, until `visit` breaks; none
    /// for a `query` of no words.
    pub fn matching(
        &self,
        query: &str,
        mut visit: impl FnMut(Listed) -> ControlFlow<()>,
    ) -> Result<(), Error> {
        let terms = search::terms(query);
        if terms.is_empty() {
            return Ok(());
        }

        let mut statement = self.connection.prepare(&format!(
            "SELECT {COLUMNS} FROM message_text JOIN message m ON m.rowid = message_text.rowid
             WHERE message_text MATCH ?1
               AND m.mailbox NOT IN (SELECT name FROM mailbox
                                     WHERE account = m.account AND role IN (?2, ?3))
             ORDER BY message_text.rowid DESC"
        ))?;
        let args = params![terms, Role::Trash.label(), Role::Spam.label()];
        let mut rows = statement.query(args)?;
        while let Some(row) = rows.next()? {
            if visit(listed(row)?).is_break() {
                break;
            }
        }

        Ok(())
    }
}

fn listed(row: &Row<'_>) -> rusqlite::Result<Listed> {
    Ok(Listed {
        account: row.get(0)?,
        folder: row.get(1)?,
        body_key: row.get(2)?,
        thread: row.get(3)?,
        root: row.get(4)?,
        received: received(row, 5)?,
        unread: row.get(7)?,
        from: row.get(8)?,
        subject: row.get(9)?,
        snippet: row.get(10)?,
    })
}
