use std::collections::HashSet;

use chrono::{DateTime, FixedOffset, Offset, Utc};
use rusqlite::{Row, params};

use super::{Cache, Error, threads::THREAD_ROW};
use crate::{
    domain::folder::Role, domain::folder_name::FolderName, domain::inbox::Inbox,
    domain::message::Message, domain::thread_id::ThreadId,
};

/// The columns `Cache::message` reads, in its order, of a `message` row.
const MESSAGE_ROW: &str = "uid, body_key, received, received_offset, unread, sender, subject,
    coalesce(snippet, ''), thread, from_me, answered, from_line, to_line";

impl Cache {
    /// Every cached Folder's Messages but Trash's and Spam's, newest first,
    /// each once: the copy in `shown` wins, else the first in Folder order.
    /// With them, their Threads' Messages found only elsewhere whose body is
    /// cached.
    pub fn everywhere(&self, shown: Option<&FolderName>) -> Result<Inbox, Error> {
        let mut folders = self.folders()?;
        folders.retain(|f| !matches!(f.role, Some(Role::Trash | Role::Spam)));
        folders.sort_by_key(|f| Some(&f.name) != shown);
        let connection = self.connection();
        let mut statement = connection.prepare_cached(&format!(
            "SELECT {MESSAGE_ROW}
             FROM message WHERE account = ?2 AND mailbox = ?1"
        ))?;
        let (mut messages, mut keys) = (Vec::new(), HashSet::new());
        for folder in &folders {
            let rows = statement
                .query_map(params![&folder.name, self.email()], |row| self.message(row))?;
            for row in rows {
                let row = row?;
                if keys.insert(row.body_key) {
                    messages.push(Message {
                        folder: Some(folder.name.clone()),
                        ..row
                    });
                }
            }
        }

        messages.sort_unstable_by(|a, b| (b.received, b.uid).cmp(&(a.received, a.uid)));
        let threads: HashSet<ThreadId> = messages.iter().filter_map(|m| m.thread).collect();
        let mut statement = connection.prepare_cached(&format!(
            "SELECT {THREAD_ROW}
             FROM thread_message t JOIN body ON body.account = t.account AND body.key = t.key
             WHERE t.account = ?1
             ORDER BY t.received, t.key"
        ))?;
        let mut elsewhere = Vec::new();
        for row in statement.query_map([self.email()], |row| self.message(row))? {
            let row = row?;
            if row.thread.is_some_and(|t| threads.contains(&t)) && !keys.contains(&row.body_key) {
                elsewhere.push(row);
            }
        }

        Ok(Inbox {
            messages,
            elsewhere,
        })
    }

    /// A row of `MESSAGE_ROW` or `THREAD_ROW` as a Message.
    fn message(&self, row: &Row<'_>) -> rusqlite::Result<Message> {
        let offset = FixedOffset::east_opt(row.get(3)?).unwrap_or_else(|| Utc.fix());
        let received = DateTime::from_timestamp(row.get(2)?, 0)
            .unwrap_or_default()
            .with_timezone(&offset);
        Ok(Message {
            uid: row.get(0)?,
            body_key: row.get(1)?,
            received,
            unread: row.get(4)?,
            sender: row.get(5)?,
            subject: row.get(6)?,
            snippet: row.get(7)?,
            thread: row.get(8)?,
            from_me: row.get(9)?,
            answered: row.get(10)?,
            from: row.get(11)?,
            to: row.get(12)?,
            folder: None,
            account: self.account(),
        })
    }
}
