use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, params};

use super::{Cache, Error, folder_key::FolderKey};
use crate::{
    domain::body_key::BodyKey, domain::flag::Flag, domain::flag_change::FlagChange,
    domain::message_ref::MessageRef, domain::uid::Uid,
};

/// A Folder's flag changes for the server.
pub type FolderFlags = (Arc<str>, Vec<FlagChange>);

impl Cache {
    /// Sets Unread on each `(folder, uid)` and on Gmail's copies of it in
    /// other Folders, adjusting their Unread counts, and queues the change
    /// for the server. Returns whether any row changed.
    pub fn mark(&self, messages: &[MessageRef], unread: bool) -> Result<bool, Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut changed = false;
            for MessageRef { folder, uid } in messages {
                changed |= mark(&transaction, self.key(folder), *uid, unread)?;
            }

            transaction.commit()?;
            Ok(changed)
        })
    }

    /// Sets `\Answered` on `uid` of `folder` and on Gmail's copies of it,
    /// and queues it for the server.
    pub fn answer(&self, folder: &str, uid: Uid) -> Result<(), Error> {
        let account = self.email();
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let key: Option<BodyKey> = transaction
                .prepare_cached(
                    "SELECT body_key FROM message WHERE account = ?3 AND mailbox = ?1 AND uid = ?2",
                )?
                .query_row(params![folder, uid, account], |row| row.get(0))
                .optional()?;
            if let Some(key) = key {
                transaction
                    .prepare_cached(
                        "UPDATE message SET answered = 1 WHERE account = ?4
                           AND ((mailbox = ?1 AND uid = ?2) OR (body_key = ?3 AND thread IS NOT NULL))",
                    )?
                    .execute(params![folder, uid, key, account])?;
                transaction
                    .prepare_cached(
                        "UPDATE thread_message SET answered = 1 WHERE account = ?2 AND key = ?1",
                    )?
                    .execute(params![key, account])?;
            }

            queue(&transaction, FolderKey { account, folder }, uid, Flag::Answered, true)?;
            transaction.commit()
        })
    }

    /// The flag changes the server has not acknowledged, by Folder.
    pub fn pending_flags(&self) -> Result<Vec<FolderFlags>, Error> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(
            "SELECT mailbox, uid, flag, added FROM flag_change WHERE account = ?1
             ORDER BY mailbox, uid, flag",
        )?;
        let mut pending: Vec<FolderFlags> = Vec::new();
        for row in statement.query_map([self.email()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get(1)?,
                row.get::<_, String>(2)?,
                row.get(3)?,
            ))
        })? {
            let (folder, uid, flag, added) = row?;
            let Some(flag) = Flag::from_name(&flag) else {
                continue;
            };

            let change = FlagChange { uid, flag, added };
            match pending.last_mut() {
                Some((last, changes)) if **last == *folder => changes.push(change),
                _ => pending.push((folder.into(), vec![change])),
            }
        }

        Ok(pending)
    }

    /// Drops `changes` of `folder` from the queue; one changed again since
    /// stays queued.
    pub fn flags_sent(&self, folder: &str, changes: &[FlagChange]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut delete = transaction.prepare_cached(
                "DELETE FROM flag_change WHERE account = ?5 AND mailbox = ?1 AND uid = ?2
                   AND flag = ?3 AND added = ?4",
            )?;
            for change in changes {
                delete.execute(params![
                    folder,
                    change.uid,
                    change.flag.name(),
                    change.added,
                    self.email()
                ])?;
            }

            drop(delete);
            transaction.commit()
        })
    }
}

/// One Message of `Cache::mark`; false when it is not cached or already so.
fn mark(
    connection: &Connection,
    key: FolderKey<'_>,
    uid: Uid,
    unread: bool,
) -> rusqlite::Result<bool> {
    let FolderKey { account, folder } = key;
    let key: Option<BodyKey> = connection
        .prepare_cached(
            "SELECT body_key FROM message WHERE account = ?4 AND mailbox = ?1 AND uid = ?2
               AND unread != ?3",
        )?
        .query_row(params![folder, uid, unread, account], |row| row.get(0))
        .optional()?;
    let Some(key) = key else {
        return Ok(false);
    };

    // Only Gmail's body key names one Message in every Folder, and its
    // `\Seen` is shared by them all.
    let copies = |account: &str| {
        format!(
            "account = {account} AND ((mailbox = ?1 AND uid = ?2) OR (body_key = ?3 AND thread IS NOT NULL))"
        )
    };
    connection
        .prepare_cached(&format!(
            "UPDATE mailbox SET unread = max(unread + ?4, 0) WHERE account = ?6
               AND name IN (SELECT mailbox FROM message WHERE ({}) AND unread != ?5)",
            copies("?6")
        ))?
        .execute(params![
            folder,
            uid,
            key,
            if unread { 1 } else { -1 },
            unread,
            account
        ])?;
    connection
        .prepare_cached(&format!(
            "UPDATE message SET unread = ?4 WHERE ({}) AND unread != ?4",
            copies("?5")
        ))?
        .execute(params![folder, uid, key, unread, account])?;
    connection
        .prepare_cached("UPDATE thread_message SET unread = ?2 WHERE account = ?3 AND key = ?1")?
        .execute(params![key, unread, account])?;
    queue(
        connection,
        FolderKey { account, folder },
        uid,
        Flag::Seen,
        !unread,
    )?;
    Ok(true)
}

/// Queues `flag` on `uid` of `folder` for the server, under the UID the
/// server holds it by after an Undo.
fn queue(
    connection: &Connection,
    key: FolderKey<'_>,
    uid: Uid,
    flag: Flag,
    added: bool,
) -> rusqlite::Result<()> {
    let FolderKey { account, folder } = key;
    connection
        .prepare_cached(
            "INSERT OR REPLACE INTO flag_change (account, mailbox, uid, flag, added)
             VALUES (?5, ?1, coalesce(
                 (SELECT at_uid FROM undone
                  WHERE account = ?5 AND mailbox = ?1 AND uid = ?2 AND NOT at_trash), ?2
             ), ?3, ?4)",
        )?
        .execute(params![folder, uid, flag.name(), added, account])?;
    Ok(())
}
