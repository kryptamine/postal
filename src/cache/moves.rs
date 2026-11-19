use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, params};

use super::{Cache, Error};

/// Queued: the server still holds the Message where it was.
const QUEUED: i64 = 0;
/// The server moved it to Trash.
const IN_TRASH: i64 = 1;
/// An Undo took it back; the server has yet to move it out of Trash.
const BACK: i64 = 2;
/// An Undo took it back while queued: unsent, unless the move was under way.
const CANCELLED: i64 = 3;
/// Back out of Trash under `back_uid`; the row shown keeps the old UID
/// until a resync fetches the new one.
const RETURNED: i64 = 4;

/// The columns `moved` keeps of a `message` row but its Folder and UID.
const ROW: &str = "body_key, received, received_offset, unread, sender, subject, thread,
    from_me, snippet, text_section, text_encoding, text_charset, text_html";

/// One Message the server is to move: to Trash for a Delete, or back for an Undo.
pub struct PendingMove {
    pub id: i64,
    /// The Folder to select, and the Message's UID there.
    pub from: Arc<str>,
    pub uid: u32,
    /// `None` for Trash, whichever Folder the server names so.
    pub to: Option<Arc<str>>,
}

impl Cache {
    /// Takes each `(folder, uid)` out of the Message List, with Gmail's
    /// copies of it under other labels, and queues its move to Trash.
    /// Returns the Delete for an Undo; `None` when no row was cached.
    pub fn delete(&self, messages: &[(Arc<str>, u32)]) -> Result<Option<i64>, Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let batch: i64 = transaction.query_row(
                "SELECT coalesce(max(batch), 0) + 1 FROM moved",
                [],
                |row| row.get(0),
            )?;
            let mut any = false;
            for (folder, uid) in messages {
                any |= delete(&transaction, batch, (folder, *uid))?;
            }

            transaction.commit()?;
            Ok(any.then_some(batch))
        })
    }

    /// Puts the Messages of Delete `batch` back in their Folders and queues
    /// their move out of Trash. Returns the Folders they went back to, none
    /// when the server cannot be asked: it named no UID in Trash.
    pub fn undo(&self, batch: i64) -> Result<Vec<Arc<str>>, Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let rows: Vec<(i64, i64, Option<String>, Option<u32>)> = transaction
                .prepare_cached("SELECT id, state, trash, trash_uid FROM moved WHERE batch = ?1")?
                .query_map([batch], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })?
                .collect::<Result<_, _>>()?;
            let mut folders: Vec<Arc<str>> = Vec::new();
            for (id, state, trash, trash_uid) in rows {
                let folder = match (state, trash, trash_uid) {
                    (QUEUED, ..) => {
                        let folder = restore(&transaction, id)?;
                        transaction.execute(
                            "UPDATE moved SET state = ?2 WHERE id = ?1",
                            [id, CANCELLED],
                        )?;
                        folder
                    }
                    (IN_TRASH, Some(trash), Some(uid)) => {
                        let folder = restore(&transaction, id)?;
                        drop_row(&transaction, (&trash, uid))?;
                        transaction
                            .execute("UPDATE moved SET state = ?2 WHERE id = ?1", [id, BACK])?;
                        folder
                    }
                    _ => {
                        transaction.execute("DELETE FROM moved WHERE id = ?1", [id])?;
                        continue;
                    }
                };

                if !folders.iter().any(|f| **f == *folder) {
                    folders.push(folder.into());
                }
            }

            transaction.commit()?;
            Ok(folders)
        })
    }

    /// Every move the server has yet to make, those back out of Trash first.
    pub fn pending_moves(&self) -> Result<Vec<PendingMove>, Error> {
        let connection = self.connection.borrow();
        let mut statement = connection.prepare_cached(
            "SELECT id, state, mailbox, uid, trash, trash_uid FROM moved
             WHERE state IN (?1, ?2) ORDER BY state DESC, mailbox, trash, uid",
        )?;
        let pending = statement
            .query_map([QUEUED, BACK], |row| {
                let (state, mailbox): (i64, String) = (row.get(1)?, row.get(2)?);
                if state == BACK {
                    return Ok(PendingMove {
                        id: row.get(0)?,
                        from: row.get::<_, String>(4)?.into(),
                        uid: row.get(5)?,
                        to: Some(mailbox.into()),
                    });
                }

                Ok(PendingMove {
                    id: row.get(0)?,
                    from: mailbox.into(),
                    uid: row.get(3)?,
                    to: None,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(pending)
    }

    /// The server moved `id` to `trash`, under `uid` there when it said.
    /// An Undo made meanwhile queues it back; false when it cannot, without
    /// `uid`, and the Message stays in Trash.
    pub fn moved(&self, id: i64, trash: &str, uid: Option<u32>) -> Result<bool, Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let state: Option<i64> = transaction
                .prepare_cached("SELECT state FROM moved WHERE id = ?1")?
                .query_row([id], |row| row.get(0))
                .optional()?;
            let late = state == Some(CANCELLED);
            if late && uid.is_none() {
                let row: (String, u32) = transaction.query_row(
                    "SELECT mailbox, uid FROM moved WHERE id = ?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                drop_row(&transaction, (&row.0, row.1))?;
                transaction.execute("DELETE FROM moved WHERE id = ?1", [id])?;
            } else if matches!(state, Some(QUEUED | CANCELLED)) {
                transaction
                    .prepare_cached(
                        "UPDATE moved SET state = ?2, trash = ?3, trash_uid = ?4 WHERE id = ?1",
                    )?
                    .execute(params![id, if late { BACK } else { IN_TRASH }, trash, uid])?;
            }

            transaction.commit()?;
            Ok(!late || uid.is_some())
        })
    }

    /// The server moved `id` back out of Trash, under `uid` there; `None`
    /// when the Message was no longer in Trash, and its row goes.
    pub fn moved_back(&self, id: i64, uid: Option<u32>) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            if uid.is_some() {
                transaction
                    .prepare_cached("UPDATE moved SET state = ?2, back_uid = ?3 WHERE id = ?1")?
                    .execute(params![id, RETURNED, uid])?;
                return transaction.commit();
            }

            let row: Option<(String, u32)> = transaction
                .prepare_cached("SELECT mailbox, uid FROM moved WHERE id = ?1")?
                .query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))
                .optional()?;
            if let Some((folder, uid)) = row {
                drop_row(&transaction, (&folder, uid))?;
            }

            transaction.execute("DELETE FROM moved WHERE id = ?1", [id])?;
            transaction.commit()
        })
    }

    /// Takes back a Delete of `id` the server refused, or could not be sent.
    pub fn unmove(&self, id: i64) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            // An Undo has put the row back already.
            let undone = transaction
                .prepare_cached("SELECT 1 FROM moved WHERE id = ?1 AND state = ?2")?
                .query_row([id, CANCELLED], |_| Ok(()))
                .optional()?;
            if undone.is_none() {
                restore(&transaction, id)?;
            }

            transaction.execute("DELETE FROM moved WHERE id = ?1", [id])?;
            transaction.commit()
        })
    }
}

/// One Message of `Cache::delete`; false when it is not cached, or still
/// on its way back from Trash under a UID the server no longer has.
fn delete(
    connection: &Connection,
    batch: i64,
    (folder, uid): (&str, u32),
) -> rusqlite::Result<bool> {
    let returning = connection
        .prepare_cached(
            "SELECT 1 FROM moved WHERE mailbox = ?1 AND uid = ?2 AND state IN (?3, ?4)",
        )?
        .query_row(params![folder, uid, BACK, CANCELLED], |_| Ok(()))
        .optional()?;
    let key: Option<(i64, Option<i64>)> = connection
        .prepare_cached("SELECT body_key, thread FROM message WHERE mailbox = ?1 AND uid = ?2")?
        .query_row(params![folder, uid], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    let Some((key, thread)) = key.filter(|_| returning.is_none()) else {
        return Ok(false);
    };

    // A row back from Trash is moved by the UID the server gave it there.
    let back: Option<u32> = connection
        .prepare_cached(
            "SELECT back_uid FROM moved WHERE mailbox = ?1 AND uid = ?2 AND state = ?3",
        )?
        .query_row(params![folder, uid, RETURNED], |row| row.get(0))
        .optional()?;
    connection
        .prepare_cached("DELETE FROM moved WHERE mailbox = ?1 AND uid = ?2 AND state = ?3")?
        .execute(params![folder, uid, RETURNED])?;
    connection
        .prepare_cached(&format!(
            "INSERT INTO moved (batch, state, mailbox, uid, {ROW})
             SELECT ?1, ?2, mailbox, ?5, {ROW} FROM message WHERE mailbox = ?3 AND uid = ?4"
        ))?
        .execute(params![batch, QUEUED, folder, uid, back.unwrap_or(uid)])?;
    // Gmail drops every label of a Message moved to Trash.
    let copies = "(mailbox = ?1 AND uid = ?2) OR (body_key = ?3 AND thread IS NOT NULL)";
    connection
        .prepare_cached(&format!(
            "UPDATE mailbox SET unread = max(unread - 1, 0)
             WHERE name IN (SELECT mailbox FROM message WHERE ({copies}) AND unread)"
        ))?
        .execute(params![folder, uid, key])?;
    connection
        .prepare_cached(&format!("DELETE FROM message WHERE {copies}"))?
        .execute(params![folder, uid, key])?;
    if thread.is_some() {
        connection
            .prepare_cached("DELETE FROM thread_message WHERE key = ?1")?
            .execute([key])?;
    }

    Ok(true)
}

/// Writes the row `moved` kept for `id` back into its Folder, and returns
/// that Folder.
fn restore(connection: &Connection, id: i64) -> rusqlite::Result<String> {
    connection
        .prepare_cached(&format!(
            "INSERT OR REPLACE INTO message (mailbox, uid, {ROW})
             SELECT mailbox, uid, {ROW} FROM moved WHERE id = ?1"
        ))?
        .execute([id])?;
    connection
        .prepare_cached(
            "UPDATE mailbox SET unread = unread + 1
             WHERE name = (SELECT mailbox FROM moved WHERE id = ?1 AND unread)",
        )?
        .execute([id])?;
    connection
        .prepare_cached("SELECT mailbox FROM moved WHERE id = ?1")?
        .query_row([id], |row| row.get(0))
}

/// Drops one row from the Message List, with its share of the Unread count.
fn drop_row(connection: &Connection, (folder, uid): (&str, u32)) -> rusqlite::Result<()> {
    connection
        .prepare_cached(
            "UPDATE mailbox SET unread = max(unread - 1, 0) WHERE name = ?1
               AND EXISTS (SELECT 1 FROM message WHERE mailbox = ?1 AND uid = ?2 AND unread)",
        )?
        .execute(params![folder, uid])?;
    connection
        .prepare_cached("DELETE FROM message WHERE mailbox = ?1 AND uid = ?2")?
        .execute(params![folder, uid])?;
    Ok(())
}
