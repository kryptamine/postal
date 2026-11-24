use std::ops::ControlFlow;

use chrono::{DateTime, FixedOffset};
use rusqlite::{Row, params};

use super::{Error, Reader, query_sql::Select, received::received};
use crate::{
    domain::body_key::BodyKey, domain::query::Query, domain::root_id::RootId,
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

    /// The Messages matching `query` in the Folders it reads, newest
    /// first, until `visit` breaks; none for a Query of no terms.
    pub fn matching(
        &self,
        query: &Query,
        mut visit: impl FnMut(Listed) -> ControlFlow<()>,
    ) -> Result<(), Error> {
        if query.terms.is_empty() {
            return Ok(());
        }

        let folders = self.folders()?;
        let places: Vec<_> = (folders.iter())
            .filter(|folder| query.reads(&folder.account, folder))
            .map(|folder| (folder.account.as_str(), folder.name.as_str()))
            .collect();
        Select::new(COLUMNS, query, &places, None)
            .rows(&self.connection, |row| Ok(visit(listed(row)?)))?;

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
