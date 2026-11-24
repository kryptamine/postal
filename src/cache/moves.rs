use rusqlite::{Connection, OptionalExtension, params};

use super::{Cache, Error, folder_key::FolderKey, schema::MOVED_ROW};
use crate::{
    domain::body_key::BodyKey, domain::folder::Role, domain::folder_name::FolderName,
    domain::message_ref::MessageRef, domain::thread_id::ThreadId, domain::uid::Uid,
};

/// One Message the server is to move, from the Folder to select.
pub struct PendingMove {
    pub id: i64,
    pub from: FolderName,
    pub uid: Uid,
    pub to: Destination,
}

/// Where a Delete, an Archive or a Move takes a Message.
#[derive(PartialEq, Eq)]
pub enum Goal {
    /// The Folder with this role, looked up when the move is sent.
    Role(Role),
    Folder(FolderName),
}

#[derive(PartialEq, Eq)]
pub enum Destination {
    Goal(Goal),
    /// For an Undo: the Folder the Message was moved from.
    Back(FolderName),
}

impl Cache {
    /// Takes each `(folder, uid)` out of the Message List, to be moved to
    /// `goal`; a move to Trash or Spam takes Gmail's copies under other
    /// labels too. Returns the move for an Undo; `None` when no row moved.
    pub fn move_to(&self, messages: &[MessageRef], goal: &Goal) -> Result<Option<i64>, Error> {
        let account = self.email();
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let batch: i64 = transaction.query_row(
                "SELECT coalesce(max(batch), 0) + 1 FROM moved",
                [],
                |row| row.get(0),
            )?;
            let goal = Target::of(&transaction, account, goal)?;
            let mut any = false;
            for MessageRef { folder, uid } in messages {
                let key = FolderKey {
                    account,
                    folder: folder.as_str(),
                };
                any |= take(&transaction, batch, &goal, key, *uid)?;
            }

            transaction.commit()?;
            Ok(any.then_some(batch))
        })
    }

    /// Puts the Messages of move `batch` back in their Folders, and returns
    /// those Folders; none when the server never named the UIDs it gave
    /// them where they went.
    pub fn undo(&self, batch: i64) -> Result<Vec<FolderName>, Error> {
        let account = self.email();
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            // Unreachable: away under a UID the server never named.
            transaction.execute(
                "DELETE FROM moved WHERE batch = ?1 AND at_away AND at_uid IS NULL",
                [batch],
            )?;
            // Gmail's All Mail keeps what an Undo copies out of it.
            let arrived: Vec<(String, Uid)> = transaction
                .prepare_cached(
                    "SELECT away, at_uid FROM moved m WHERE batch = ?1 AND want_away AND at_away
                       AND NOT (thread IS NOT NULL AND EXISTS (SELECT 1 FROM mailbox b
                         WHERE b.account = m.account AND b.name = m.away AND b.role = ?2))",
                )?
                .query_map(params![batch, Role::All.label()], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?
                .collect::<Result<_, _>>()?;
            for (away, uid) in arrived {
                drop_row(
                    &transaction,
                    FolderKey {
                        account,
                        folder: &away,
                    },
                    uid,
                )?;
            }

            let rows: Vec<(i64, FolderName)> = transaction
                .prepare_cached("SELECT id, mailbox FROM moved WHERE batch = ?1 AND want_away")?
                .query_map([batch], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            let mut folders: Vec<FolderName> = Vec::new();
            for (id, folder) in rows {
                restore(&transaction, id)?;
                if !folders.contains(&folder) {
                    folders.push(folder);
                }
            }

            transaction.execute("UPDATE moved SET want_away = 0 WHERE batch = ?1", [batch])?;
            transaction.commit()?;
            Ok(folders)
        })
    }

    /// Every move the server has yet to make, those back first.
    pub fn pending_moves(&self) -> Result<Vec<PendingMove>, Error> {
        let connection = self.connection();
        let mut statement = connection.prepare_cached(
            "SELECT id, want_away, mailbox, away, at_uid, to_role, to_folder FROM moved
             WHERE account = ?1 AND want_away != at_away AND at_uid IS NOT NULL
             ORDER BY want_away,
                 CASE WHEN want_away THEN mailbox ELSE away END,
                 CASE WHEN want_away THEN coalesce('r' || to_role, 'f' || to_folder)
                      ELSE mailbox END,
                 at_uid",
        )?;
        let pending = statement
            .query_map([self.email()], |row| {
                let (id, want_away, mailbox): (i64, bool, String) =
                    (row.get(0)?, row.get(1)?, row.get(2)?);
                let uid = row.get(4)?;
                Ok(if want_away {
                    let role: Option<String> = row.get(5)?;
                    let goal = match role.as_deref().and_then(Role::from_label) {
                        Some(role) => Goal::Role(role),
                        None => Goal::Folder(row.get::<_, String>(6)?.into()),
                    };

                    PendingMove {
                        id,
                        from: mailbox.into(),
                        uid,
                        to: Destination::Goal(goal),
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
        to: &FolderName,
        uid: Option<Uid>,
    ) -> Result<bool, Error> {
        let away = matches!(pending.to, Destination::Goal(_));
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            transaction
                .prepare_cached(
                    "UPDATE moved SET at_away = ?2, at_uid = ?3,
                         away = CASE WHEN ?2 THEN ?4 ELSE away END
                     WHERE id = ?1",
                )?
                .execute(params![pending.id, away, uid, to])?;
            // Back where it was, or away unnamed: nothing can move it now.
            let reached = uid.is_some() || !wanted_back(&transaction, pending.id)?;
            if uid.is_none() {
                lose(&transaction, pending.id)?;
            }

            transaction.commit()?;
            Ok(reached)
        })
    }

    /// The server refused `pending`: the Message stays where it is, shown
    /// again for a move away and out of reach for an Undo. False for the latter.
    pub fn refused(&self, pending: &PendingMove) -> Result<bool, Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let back = wanted_back(&transaction, pending.id)?;
            let away = matches!(pending.to, Destination::Goal(_));
            if !away {
                lose(&transaction, pending.id)?;
            } else if !back {
                restore(&transaction, pending.id)?;
            }

            transaction.execute("DELETE FROM moved WHERE id = ?1", [pending.id])?;
            transaction.commit()?;
            Ok(away || !back)
        })
    }
}

/// Once its new UID is in `folder`, an Undo's row gives way to it.
pub(super) fn settle(connection: &Connection, key: FolderKey<'_>) -> rusqlite::Result<()> {
    let FolderKey { account, folder } = key;
    let arrived = "NOT at_away AND at_uid != uid
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
            "DELETE FROM moved WHERE account = ?2 AND mailbox = ?1 AND NOT want_away
               AND {arrived}"
        ),
        params![folder, account],
    )?;
    Ok(())
}

/// Drops what an Undo of an earlier run might have needed: the moves the
/// server made, and those undone before it saw them.
pub(super) fn forget_settled(connection: &Connection) -> rusqlite::Result<()> {
    connection
        .execute(
            "DELETE FROM moved WHERE want_away = at_away AND (at_away OR at_uid = uid)",
            [],
        )
        .map(drop)
}

/// A `Goal` as `moved` stores it, with what the Cache knows of its Folder.
struct Target<'a> {
    role: Option<Role>,
    folder: Option<&'a str>,
    /// Gmail drops every label of a Message moved to Trash or Spam.
    drops_labels: bool,
}

impl<'a> Target<'a> {
    fn of(connection: &Connection, account: &str, goal: &'a Goal) -> rusqlite::Result<Self> {
        let (role, folder) = match goal {
            Goal::Role(role) => (Some(*role), None),
            Goal::Folder(folder) => {
                let role: Option<String> = connection
                    .prepare_cached("SELECT role FROM mailbox WHERE account = ?1 AND name = ?2")?
                    .query_row(params![account, folder], |row| row.get(0))
                    .optional()?
                    .flatten();
                (
                    role.as_deref().and_then(Role::from_label),
                    Some(folder.as_str()),
                )
            }
        };

        Ok(Self {
            role: folder.is_none().then_some(role).flatten(),
            folder,
            drops_labels: matches!(role, Some(Role::Trash | Role::Spam)),
        })
    }
}

/// One Message of `Cache::move_to`; false when it is not cached, in the
/// goal already, in All Mail bound for a label, or on its way back from
/// another move.
fn take(
    connection: &Connection,
    batch: i64,
    goal: &Target<'_>,
    key: FolderKey<'_>,
    uid: Uid,
) -> rusqlite::Result<bool> {
    let FolderKey { account, folder } = key;
    let key: Option<(BodyKey, Option<ThreadId>, Option<String>)> = connection
        .prepare_cached(
            "SELECT body_key, thread, (SELECT role FROM mailbox b
                                       WHERE b.account = m.account AND b.name = m.mailbox)
             FROM message m WHERE account = ?3 AND mailbox = ?1 AND uid = ?2",
        )?
        .query_row(params![folder, uid, account], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .optional()?;
    let Some((key, thread, role)) = key else {
        return Ok(false);
    };

    let here = role.as_deref().and_then(Role::from_label);
    let arrived = goal
        .folder
        .map_or_else(|| goal.role == here, |f| f == folder);
    if arrived || (here == Some(Role::All) && !goal.drops_labels) {
        return Ok(false);
    }

    if !queue(connection, batch, goal, FolderKey { account, folder }, uid)? {
        return Ok(false);
    }

    if goal.drops_labels {
        drop_copies(connection, FolderKey { account, folder }, uid, key, thread)?;
    } else {
        drop_row(connection, FolderKey { account, folder }, uid)?;
    }

    Ok(true)
}

/// Writes `uid` of `folder` into `moved`, wanted in `goal`; false when it
/// is on its way back from another goal, and the server still has it there.
fn queue(
    connection: &Connection,
    batch: i64,
    goal: &Target<'_>,
    key: FolderKey<'_>,
    uid: Uid,
) -> rusqlite::Result<bool> {
    let FolderKey { account, folder } = key;
    // A row an Undo put back is wanted away again, from wherever it is.
    let undone: Option<bool> = connection
        .prepare_cached(
            "SELECT at_away AND (to_role, to_folder) IS NOT (?3, ?4) FROM moved
             WHERE account = ?5 AND mailbox = ?1 AND uid = ?2 AND NOT want_away",
        )?
        .query_row(
            params![
                folder,
                uid,
                goal.role.map(Role::label),
                goal.folder,
                account
            ],
            |row| row.get(0),
        )
        .optional()?;
    match undone {
        Some(true) => return Ok(false),
        Some(false) => connection
            .prepare_cached(
                "UPDATE moved SET want_away = 1, batch = ?3, to_role = ?4, to_folder = ?5
                 WHERE account = ?6 AND mailbox = ?1 AND uid = ?2 AND NOT want_away",
            )?
            .execute(params![
                folder,
                uid,
                batch,
                goal.role.map(Role::label),
                goal.folder,
                account
            ])?,
        None => connection
            .prepare_cached(&format!(
                "INSERT INTO moved (batch, to_role, to_folder, want_away, at_away, at_uid,
                                    mailbox, uid, {MOVED_ROW})
                 SELECT ?1, ?5, ?6, 1, 0, uid, mailbox, uid, {MOVED_ROW} FROM message
                 WHERE account = ?4 AND mailbox = ?2 AND uid = ?3"
            ))?
            .execute(params![
                batch,
                folder,
                uid,
                account,
                goal.role.map(Role::label),
                goal.folder
            ])?,
    };

    Ok(true)
}

/// Drops `uid` of `folder` with Gmail's copies of it under other labels.
fn drop_copies(
    connection: &Connection,
    key: FolderKey<'_>,
    uid: Uid,
    body_key: BodyKey,
    thread: Option<ThreadId>,
) -> rusqlite::Result<()> {
    let FolderKey { account, folder } = key;
    drop_rows(
        connection,
        "account = ?4 AND ((mailbox = ?1 AND uid = ?2) OR (body_key = ?3 AND thread IS NOT NULL))",
        params![folder, uid, body_key, account],
    )?;
    if thread.is_some() {
        connection
            .prepare_cached("DELETE FROM thread_message WHERE account = ?2 AND key = ?1")?
            .execute(params![body_key, account])?;
    }

    Ok(())
}

fn wanted_back(connection: &Connection, id: i64) -> rusqlite::Result<bool> {
    let back = connection
        .prepare_cached("SELECT NOT want_away FROM moved WHERE id = ?1")?
        .query_row([id], |row| row.get(0))
        .optional()?;
    Ok(back.unwrap_or(false))
}

/// Forgets `id` for good, with the row an Undo showed for it.
fn lose(connection: &Connection, id: i64) -> rusqlite::Result<()> {
    let shown: Option<(String, String, Uid)> = connection
        .prepare_cached("SELECT account, mailbox, uid FROM moved WHERE id = ?1 AND NOT want_away")?
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
            "INSERT OR REPLACE INTO message (mailbox, uid, {MOVED_ROW})
             SELECT mailbox, uid, {MOVED_ROW} FROM moved WHERE id = ?1"
        ))?
        .execute([id])?;
    connection
        .prepare_cached(
            "UPDATE mailbox SET messages = messages + 1,
                 unread = unread + (SELECT unread FROM moved WHERE id = ?1)
             WHERE (account, name) = (SELECT account, mailbox FROM moved WHERE id = ?1)",
        )?
        .execute([id])?;
    Ok(())
}

/// Drops one row from the Message List.
fn drop_row(connection: &Connection, key: FolderKey<'_>, uid: Uid) -> rusqlite::Result<()> {
    let FolderKey { account, folder } = key;
    drop_rows(
        connection,
        "account = ?3 AND mailbox = ?1 AND uid = ?2",
        params![folder, uid, account],
    )
}

/// Deletes the message rows `rows` matches, each Folder's counts down by
/// its share: one per Folder, as a Folder holds a Message once.
fn drop_rows(
    connection: &Connection,
    rows: &str,
    params: &[&dyn rusqlite::ToSql],
) -> rusqlite::Result<()> {
    connection
        .prepare_cached(&format!(
            "UPDATE mailbox SET messages = max(messages - 1, 0),
                 unread = max(unread - EXISTS (SELECT 1 FROM message m WHERE ({rows})
                                                 AND m.account = mailbox.account
                                                 AND m.mailbox = mailbox.name AND m.unread), 0)
               WHERE (account, name) IN (SELECT account, mailbox FROM message WHERE {rows})"
        ))?
        .execute(params)?;
    connection
        .prepare_cached(&format!("DELETE FROM message WHERE {rows}"))?
        .execute(params)?;
    Ok(())
}
