use chrono::{DateTime, FixedOffset};
use rusqlite::{OptionalExtension, params};

use super::{Error, Reader, received::received};
use crate::{
    domain::attachment::Attachments, domain::body_key::BodyKey, domain::folder::Role,
    domain::root_id::RootId, domain::thread_id::ThreadId,
};

/// One Message as the Cache holds it, with its body when cached.
pub struct MessageRow {
    /// `None` for one only All Mail held when its Thread was last read.
    pub folder: Option<String>,
    pub thread: Option<ThreadId>,
    pub root: Option<RootId>,
    /// As `Message::from` and `Message::to` hold them.
    pub from: String,
    pub to: String,
    pub subject: String,
    pub received: DateTime<FixedOffset>,
    pub attachments: Attachments,
    pub raw: Option<Vec<u8>>,
}

impl Reader {
    /// `account`'s Message `key`: its copy outside Trash and Spam first,
    /// else the one its Thread holds; `None` when the Cache has neither.
    pub fn message(&self, account: &str, key: BodyKey) -> Result<Option<MessageRow>, Error> {
        let found = self
            .connection
            .query_row(
                "SELECT * FROM (
                     SELECT mailbox, thread, root, received, received_offset, from_line,
                            to_line, subject, attachments
                     FROM message WHERE account = ?1 AND body_key = ?2
                     UNION ALL
                     SELECT NULL, thread, NULL, received, received_offset, from_line,
                            to_line, subject, attachments
                     FROM thread_message WHERE account = ?1 AND key = ?2)
                 ORDER BY mailbox IS NULL,
                          mailbox IN (SELECT name FROM mailbox
                                      WHERE account = ?1 AND role IN (?3, ?4))
                 LIMIT 1",
                params![account, key, Role::Trash.label(), Role::Spam.label()],
                |row| {
                    let mut attachments = Attachments::default();
                    attachments.stored_mut().push_str(row.get_ref(8)?.as_str()?);
                    Ok(MessageRow {
                        folder: row.get(0)?,
                        thread: row.get(1)?,
                        root: row.get(2)?,
                        received: received(row, 3)?,
                        from: row.get(5)?,
                        to: row.get(6)?,
                        subject: row.get(7)?,
                        attachments,
                        raw: None,
                    })
                },
            )
            .optional()?;
        let Some(mut found) = found else {
            return Ok(None);
        };

        found.raw = self.body(account, key)?;
        Ok(Some(found))
    }
}
