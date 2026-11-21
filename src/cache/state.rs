use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use super::{Cache, Changes, Error, correspondence, moves};
use crate::{flag::Flag, new_message::NewMessage, sync_state::SyncState, uid::Uid};

impl Cache {
    /// `None` before the first resync of `folder`.
    pub fn state(&self, folder: &str) -> Result<Option<SyncState>, Error> {
        let state = self
            .connection()
            .prepare_cached(
                "SELECT uid_validity, uid_next, highest_modseq FROM mailbox
                 WHERE account = ?2 AND name = ?1 AND uid_validity IS NOT NULL",
            )?
            .query_row(params![folder, self.email()], |row| {
                Ok(SyncState {
                    uid_validity: row.get(0)?,
                    uid_next: row.get(1)?,
                    highest_modseq: row.get(2)?,
                })
            })
            .optional()?;
        Ok(state)
    }

    /// Every UID of `folder` the server holds as the Cache knows it,
    /// ascending: a queued Delete's too, an Undo's old one not.
    pub fn uids(&self, folder: &str) -> Result<Vec<Uid>, Error> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(
            "SELECT uid FROM message WHERE account = ?2 AND mailbox = ?1
               AND NOT EXISTS (SELECT 1 FROM undone u
                 WHERE u.account = ?2 AND u.mailbox = ?1 AND u.uid = message.uid
                   AND (u.at_trash OR u.at_uid != u.uid))
             UNION SELECT uid FROM deleted WHERE account = ?2 AND mailbox = ?1
             ORDER BY uid",
        )?;
        let uids = statement
            .query_map(params![folder, self.email()], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(uids)
    }

    /// Whether the Cache held every Message of `folder` when the server last
    /// counted them: then the server has nothing older than the Cache.
    pub fn whole(&self, folder: &str) -> Result<bool, Error> {
        let exists: Option<usize> = self
            .connection()
            .prepare_cached("SELECT exists_count FROM mailbox WHERE account = ?2 AND name = ?1")?
            .query_row(params![folder, self.email()], |row| row.get(0))
            .optional()?
            .flatten();
        let Some(exists) = exists else {
            return Ok(false);
        };

        Ok(self.uids(folder)?.len() >= exists)
    }

    pub fn oldest_uid(&self, folder: &str) -> Result<Option<Uid>, Error> {
        let oldest = self
            .connection()
            .prepare_cached("SELECT min(uid) FROM message WHERE account = ?2 AND mailbox = ?1")?
            .query_row(params![folder, self.email()], |row| row.get(0))?;
        Ok(oldest)
    }

    /// When the oldest cached Message of `folder` arrived.
    pub fn oldest_arrival(&self, folder: &str) -> Result<Option<DateTime<Utc>>, Error> {
        let oldest: Option<i64> = self
            .connection()
            .prepare_cached(
                "SELECT min(received) FROM message WHERE account = ?2 AND mailbox = ?1",
            )?
            .query_row(params![folder, self.email()], |row| row.get(0))?;
        Ok(oldest.and_then(|at| DateTime::from_timestamp(at, 0)))
    }

    pub fn apply(&self, folder: &str, changes: &Changes) -> Result<(), Error> {
        self.write(|connection| apply(connection, (self.email(), folder), changes))
    }

    /// Adds rows older than every cached one in one transaction, leaving
    /// the Folder's state as the last resync wrote it.
    pub fn backfill(&self, folder: &str, older: &[NewMessage]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            insert(&transaction, (self.email(), folder), older)?;
            transaction.commit()
        })
    }
}

fn apply(
    connection: &Connection,
    (account, folder): (&str, &str),
    changes: &Changes,
) -> rusqlite::Result<()> {
    let transaction = connection.unchecked_transaction()?;
    if changes.replace {
        for table in ["message", "flag_change", "moved"] {
            transaction.execute(
                &format!("DELETE FROM {table} WHERE account = ?2 AND mailbox = ?1"),
                params![folder, account],
            )?;
        }
    }

    insert(&transaction, (account, folder), &changes.new)?;
    moves::settle(&transaction, (account, folder))?;
    // A row an Undo put back stays: the expunge is its move's.
    let mut delete = transaction.prepare_cached(
        "DELETE FROM message WHERE account = ?3 AND mailbox = ?1 AND uid = ?2
           AND NOT EXISTS (SELECT 1 FROM undone WHERE account = ?3 AND mailbox = ?1 AND uid = ?2)",
    )?;
    for uid in &changes.expunged {
        delete.execute(params![folder, uid, account])?;
    }

    // The server's flag is older than a change still queued for it.
    let flag = |column: &str| {
        transaction.prepare_cached(&format!(
            "UPDATE message SET {column} = ?3 WHERE account = ?5 AND mailbox = ?1 AND uid = ?2
               AND NOT EXISTS (SELECT 1 FROM flag_change WHERE account = ?5 AND mailbox = ?1
                                 AND uid = ?2 AND flag = ?4)"
        ))
    };
    let (mut seen, mut answered) = (flag("unread")?, flag("answered")?);
    for (uid, flags) in &changes.flags {
        seen.execute(params![
            folder,
            uid,
            flags.unread,
            Flag::Seen.name(),
            account
        ])?;
        answered.execute(params![
            folder,
            uid,
            flags.answered,
            Flag::Answered.name(),
            account
        ])?;
    }

    write_state(&transaction, (account, folder), changes)?;
    drop((delete, seen, answered));
    transaction.commit()
}

fn write_state(
    connection: &Connection,
    (account, folder): (&str, &str),
    changes: &Changes,
) -> rusqlite::Result<()> {
    let state = &changes.state;
    connection.execute(
        "INSERT INTO mailbox
             (account, name, uid_validity, uid_next, highest_modseq, unread, exists_count)
         VALUES (?6, ?1, ?2, ?3, ?4, ?5, ?7)
         ON CONFLICT (account, name) DO UPDATE SET uid_validity = excluded.uid_validity,
             uid_next = excluded.uid_next, highest_modseq = excluded.highest_modseq,
             unread = excluded.unread, exists_count = excluded.exists_count",
        params![
            folder,
            state.uid_validity,
            state.uid_next,
            state.highest_modseq,
            changes.unread,
            account,
            changes.exists,
        ],
    )?;
    Ok(())
}

/// A row whose Delete the server has yet to see stays out: the server
/// still lists it in `folder`.
fn insert(
    connection: &Connection,
    (account, folder): (&str, &str),
    rows: &[NewMessage],
) -> rusqlite::Result<()> {
    let mut insert = connection.prepare_cached(
        "INSERT OR REPLACE INTO message
         (uid, body_key, received, received_offset, unread, sender, subject, snippet,
          text_section, text_encoding, text_charset, text_html, mailbox, thread, from_me,
          answered, account, from_line, to_line)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                ?18, ?19
         WHERE NOT EXISTS (SELECT 1 FROM deleted WHERE account = ?17 AND mailbox = ?13
                             AND uid = ?1)",
    )?;
    for new in rows {
        let (m, part) = (&new.row, new.text_part.as_ref());
        insert.execute(params![
            m.uid,
            m.body_key,
            m.received.timestamp(),
            m.received.offset().local_minus_utc(),
            m.unread,
            m.sender,
            m.subject,
            part.is_none().then_some(""),
            part.map(|p| &p.section),
            part.map(|p| p.encoding.label()),
            part.map(|p| &p.charset),
            part.map(|p| p.html),
            folder,
            m.thread,
            m.from_me,
            m.answered,
            account,
            m.from,
            m.to,
        ])?;
    }

    drop(insert);
    correspondence::learn(connection, (account, folder), rows)
}
