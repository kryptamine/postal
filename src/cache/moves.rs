use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, params};

use super::{Cache, Error, folder_key::FolderKey};
use crate::{body_key::BodyKey, message_ref::MessageRef, thread_id::ThreadId, uid::Uid};

/// The columns `moved` keeps of a `message` row but its Folder and UID.
const ROW: &str = "account, body_key, received, received_offset, unread, answered, sender,
    from_line, to_line, subject, thread, from_me, snippet, text_section, text_encoding,
    text_charset, text_html";

/// One Message the server is to move, from the Folder to select.
pub struct PendingMove {
    pub id: i64,
    pub from: Arc<str>,
    pub uid: Uid,
    pub to: Destination,
}

#[derive(PartialEq, Eq)]
pub enum Destination {
    /// For a Delete: whichever Folder the server names Trash.
    Trash,
    /// For an Undo: the Folder the Message was deleted from.
    Back(Arc<str>),
}

impl Cache {
    /// Takes each `(folder, uid)` out of the Message List, with Gmail's
    /// copies of it under other labels, and wants it in Trash. Returns the
    /// Delete for an Undo; `None` when no row was cached.
    pub fn delete(&self, messages: &[MessageRef]) -> Result<Option<i64>, Error> {
        let account = self.email();
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let batch: i64 = transaction.query_row(
                "SELECT coalesce(max(batch), 0) + 1 FROM moved",
                [],
                |row| row.get(0),
            )?;
            let mut any = false;
            for MessageRef { folder, uid } in messages {
                any |= delete(&transaction, batch, FolderKey { account, folder }, *uid)?;
            }

            transaction.commit()?;
            Ok(any.then_some(batch))
        })
    }

    /// Puts the Messages of Delete `batch` back in their Folders. Returns
    /// the Folders they went back to; none when the server named no UID in
    /// Trash to move them back by.
    pub fn undo(&self, batch: i64) -> Result<Vec<Arc<str>>, Error> {
        let account = self.email();
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            // Unreachable: in Trash under a UID the server never named.
            transaction.execute(
                "DELETE FROM moved WHERE batch = ?1 AND at_trash AND at_uid IS NULL",
                [batch],
            )?;
            let trashed: Vec<(String, Uid)> = transaction
                .prepare_cached(
                    "SELECT trash, at_uid FROM moved WHERE batch = ?1 AND want_trash AND at_trash",
                )?
                .query_map([batch], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            for (trash, uid) in trashed {
                drop_row(
                    &transaction,
                    FolderKey {
                        account,
                        folder: &trash,
                    },
                    uid,
                )?;
            }

            let rows: Vec<(i64, String)> = transaction
                .prepare_cached("SELECT id, mailbox FROM moved WHERE batch = ?1 AND want_trash")?
                .query_map([batch], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            let mut folders: Vec<Arc<str>> = Vec::new();
            for (id, folder) in rows {
                restore(&transaction, id)?;
                if !folders.iter().any(|f| **f == *folder) {
                    folders.push(folder.into());
                }
            }

            transaction.execute("UPDATE moved SET want_trash = 0 WHERE batch = ?1", [batch])?;
            transaction.commit()?;
            Ok(folders)
        })
    }

    /// Every move the server has yet to make, those back out of Trash first.
    pub fn pending_moves(&self) -> Result<Vec<PendingMove>, Error> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(
            "SELECT id, want_trash, mailbox, trash, at_uid FROM moved
             WHERE account = ?1 AND want_trash != at_trash AND at_uid IS NOT NULL
             ORDER BY want_trash, mailbox, trash, at_uid",
        )?;
        let pending = statement
            .query_map([self.email()], |row| {
                let (id, want_trash, mailbox): (i64, bool, String) =
                    (row.get(0)?, row.get(1)?, row.get(2)?);
                let uid = row.get(4)?;
                Ok(if want_trash {
                    PendingMove {
                        id,
                        from: mailbox.into(),
                        uid,
                        to: Destination::Trash,
                    }
                } else {
                    PendingMove {
                        id,
                        from: row.get::<_, String>(3)?.into(),
                        uid,
                        to: Destination::Back(mailbox.into()),
                    }
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(pending)
    }

    /// The server made `pending`, into `to` under `uid` there when it said.
    /// False when the Message is out of reach of the Undo that wants it back.
    pub fn arrived(
        &self,
        pending: &PendingMove,
        to: &str,
        uid: Option<Uid>,
    ) -> Result<bool, Error> {
        let in_trash = pending.to == Destination::Trash;
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction
                .prepare_cached(
                    "UPDATE moved SET at_trash = ?2, at_uid = ?3,
                         trash = CASE WHEN ?2 THEN ?4 ELSE trash END
                     WHERE id = ?1",
                )?
                .execute(params![pending.id, in_trash, uid, to])?;
            // Gone from Trash, or into it unnamed: nothing can move it now.
            let reached = uid.is_some() || !wanted_back(&transaction, pending.id)?;
            if uid.is_none() {
                lose(&transaction, pending.id)?;
            }

            transaction.commit()?;
            Ok(reached)
        })
    }

    /// The server refused `pending`: the Message stays where it is, shown
    /// again for a Delete and out of reach for an Undo. False for the latter.
    pub fn refused(&self, pending: &PendingMove) -> Result<bool, Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let back = wanted_back(&transaction, pending.id)?;
            let into_trash = pending.to == Destination::Trash;
            if !into_trash {
                lose(&transaction, pending.id)?;
            } else if !back {
                restore(&transaction, pending.id)?;
            }

            transaction.execute("DELETE FROM moved WHERE id = ?1", [pending.id])?;
            transaction.commit()?;
            Ok(into_trash || !back)
        })
    }
}

/// Once its new UID is in `folder`, an Undo's row gives way to it.
pub(super) fn settle(connection: &Connection, key: FolderKey<'_>) -> rusqlite::Result<()> {
    let FolderKey { account, folder } = key;
    let arrived = "NOT at_trash AND at_uid != uid
        AND EXISTS (SELECT 1 FROM message n WHERE n.account = ?2 AND n.mailbox = ?1
                      AND n.uid = at_uid)";
    connection.execute(
        &format!(
            "DELETE FROM message WHERE account = ?2 AND mailbox = ?1 AND uid IN
                 (SELECT uid FROM undone WHERE account = ?2 AND mailbox = ?1 AND {arrived})"
        ),
        params![folder, account],
    )?;
    connection.execute(
        &format!(
            "DELETE FROM moved WHERE account = ?2 AND mailbox = ?1 AND NOT want_trash
               AND {arrived}"
        ),
        params![folder, account],
    )?;
    Ok(())
}

/// Drops what an Undo of an earlier run might have needed: the Deletes the
/// server made, and those undone before it saw them.
pub(super) fn forget_settled(connection: &Connection) -> rusqlite::Result<()> {
    connection
        .execute(
            "DELETE FROM moved WHERE want_trash = at_trash AND (at_trash OR at_uid = uid)",
            [],
        )
        .map(drop)
}

/// One Message of `Cache::delete`; false when it is not cached.
fn delete(
    connection: &Connection,
    batch: i64,
    key: FolderKey<'_>,
    uid: Uid,
) -> rusqlite::Result<bool> {
    let FolderKey { account, folder } = key;
    let key: Option<(BodyKey, Option<ThreadId>)> = connection
        .prepare_cached(
            "SELECT body_key, thread FROM message WHERE account = ?3 AND mailbox = ?1
               AND uid = ?2",
        )?
        .query_row(params![folder, uid, account], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?;
    let Some((key, thread)) = key else {
        return Ok(false);
    };

    // A row an Undo put back is wanted in Trash again, wherever it is.
    let again = connection
        .prepare_cached(
            "UPDATE moved SET want_trash = 1, batch = ?3
             WHERE account = ?4 AND mailbox = ?1 AND uid = ?2 AND NOT want_trash",
        )?
        .execute(params![folder, uid, batch, account])?;
    if again == 0 {
        connection
            .prepare_cached(&format!(
                "INSERT INTO moved (batch, want_trash, at_trash, at_uid, mailbox, uid, {ROW})
                 SELECT ?1, 1, 0, uid, mailbox, uid, {ROW} FROM message
                 WHERE account = ?4 AND mailbox = ?2 AND uid = ?3"
            ))?
            .execute(params![batch, folder, uid, account])?;
    }

    // Gmail drops every label of a Message moved to Trash.
    let copies =
        "account = ?4 AND ((mailbox = ?1 AND uid = ?2) OR (body_key = ?3 AND thread IS NOT NULL))";
    connection
        .prepare_cached(&format!(
            "UPDATE mailbox SET unread = max(unread - 1, 0) WHERE account = ?4
               AND name IN (SELECT mailbox FROM message WHERE ({copies}) AND unread)"
        ))?
        .execute(params![folder, uid, key, account])?;
    connection
        .prepare_cached(&format!("DELETE FROM message WHERE {copies}"))?
        .execute(params![folder, uid, key, account])?;
    if thread.is_some() {
        connection
            .prepare_cached("DELETE FROM thread_message WHERE account = ?2 AND key = ?1")?
            .execute(params![key, account])?;
    }

    Ok(true)
}

fn wanted_back(connection: &Connection, id: i64) -> rusqlite::Result<bool> {
    let back = connection
        .prepare_cached("SELECT NOT want_trash FROM moved WHERE id = ?1")?
        .query_row([id], |row| row.get(0))
        .optional()?;
    Ok(back.unwrap_or(false))
}

/// Forgets `id` for good, with the row an Undo showed for it.
fn lose(connection: &Connection, id: i64) -> rusqlite::Result<()> {
    let shown: Option<(String, String, Uid)> = connection
        .prepare_cached("SELECT account, mailbox, uid FROM moved WHERE id = ?1 AND NOT want_trash")?
        .query_row([id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .optional()?;
    if let Some((account, folder, uid)) = shown {
        drop_row(
            connection,
            FolderKey {
                account: &account,
                folder: &folder,
            },
            uid,
        )?;
    }

    connection.execute("DELETE FROM moved WHERE id = ?1", [id])?;
    Ok(())
}

/// Writes the row `moved` kept for `id` back into its Folder.
fn restore(connection: &Connection, id: i64) -> rusqlite::Result<()> {
    connection
        .prepare_cached(&format!(
            "INSERT OR REPLACE INTO message (mailbox, uid, {ROW})
             SELECT mailbox, uid, {ROW} FROM moved WHERE id = ?1"
        ))?
        .execute([id])?;
    connection
        .prepare_cached(
            "UPDATE mailbox SET unread = unread + 1
             WHERE (account, name) =
                 (SELECT account, mailbox FROM moved WHERE id = ?1 AND unread)",
        )?
        .execute([id])?;
    Ok(())
}

/// Drops one row from the Message List, with its share of the Unread count.
fn drop_row(connection: &Connection, key: FolderKey<'_>, uid: Uid) -> rusqlite::Result<()> {
    let FolderKey { account, folder } = key;
    connection
        .prepare_cached(
            "UPDATE mailbox SET unread = max(unread - 1, 0) WHERE account = ?3 AND name = ?1
               AND EXISTS (SELECT 1 FROM message WHERE account = ?3 AND mailbox = ?1
                             AND uid = ?2 AND unread)",
        )?
        .execute(params![folder, uid, account])?;
    connection
        .prepare_cached("DELETE FROM message WHERE account = ?3 AND mailbox = ?1 AND uid = ?2")?
        .execute(params![folder, uid, account])?;
    Ok(())
}
