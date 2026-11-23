use std::collections::HashSet;

use rusqlite::{OptionalExtension, params};

use super::{Cache, Error};
use crate::{
    domain::folder_name::FolderName, domain::thread::Key, domain::thread_row::ThreadRow,
    domain::uid::Uid,
};

impl Cache {
    /// Appends the Threads of `folder` to `into`, newest first, the newest
    /// `limit` of them or all; how many Messages the Folder holds, `None`
    /// before its first resync has written it.
    pub fn thread_rows(
        &self,
        folder: &FolderName,
        limit: Option<usize>,
        into: &mut Vec<ThreadRow>,
    ) -> Result<Option<usize>, Error> {
        let connection = self.connection();
        let synced = connection
            .prepare_cached(
                "SELECT 1 FROM mailbox WHERE account = ?2 AND name = ?1
                   AND uid_validity IS NOT NULL",
            )?
            .query_row(params![folder, self.email()], |_| Ok(()))
            .optional()?;
        if synced.is_none() {
            return Ok(None);
        }

        // An index scan alone: `message_by_arrival` covers these columns.
        let mut statement = connection.prepare_cached(
            "SELECT thread, body_key, received, uid, root FROM message
             WHERE account = ?2 AND mailbox = ?1
             ORDER BY received DESC, uid DESC",
        )?;
        let mut rows = statement.query(params![folder, self.email()])?;
        let mut seen = HashSet::with_capacity(into.capacity());
        let end = limit.map_or(usize::MAX, |limit| into.len() + limit);
        let mut messages = 0;
        // Past `limit` it counts on: the cache edge needs the whole Folder's.
        while let Some(row) = rows.next()? {
            messages += 1;
            if into.len() == end {
                continue;
            }

            let key = Key::new(self.account(), row.get(0)?, row.get(4)?, row.get(1)?);

            if !matches!(key, Key::Lone(..)) && !seen.insert(key) {
                continue;
            }

            into.push(ThreadRow {
                key,
                newest: (row.get(2)?, row.get::<_, Uid>(3)?),
            });
        }

        Ok(Some(messages))
    }
}
