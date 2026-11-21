use std::{
    cell::{Cell, Ref, RefCell},
    collections::HashSet,
    path::Path,
    rc::Rc,
    sync::Arc,
};

use chrono::{DateTime, FixedOffset, Offset, Utc};
use rusqlite::{
    Connection, OptionalExtension, Row,
    backup::{Backup, StepResult},
    params,
};

use super::{Changes, Error, correspondence, moves, schema};
use crate::{
    flag::Flag,
    flag_change::FlagChange,
    folder::{Folder, Role},
    inbox::Inbox,
    message::Message,
    new_message::NewMessage,
    sync_state::SyncState,
    text_part::TextPart,
    transfer_encoding::TransferEncoding,
};

/// The raw bodies kept, in bytes; past it the least recently used go first.
const BODY_BYTES: i64 = 200 * 1024 * 1024;

/// A Folder's flag changes for the server.
pub type FolderFlags = (Arc<str>, Vec<FlagChange>);

/// One Account's rows of the Cache, which every Account shares; each row
/// names its Account by address.
#[derive(Clone)]
pub struct Cache {
    shared: Rc<Shared>,
    /// The Account's place in this run.
    account: usize,
    email: Arc<str>,
}

/// Once a write to disk fails, the Cache moves into memory for the rest of
/// the session, so a full disk or a lost permission stops nothing.
struct Shared {
    connection: RefCell<Connection>,
    rebuilt: bool,
    in_memory: Cell<bool>,
    unreported_failure: Cell<bool>,
}

impl Cache {
    /// The Cache in `dir`, one handle per address of `accounts`, in order.
    pub fn open(dir: &Path, accounts: &[&str]) -> Result<Vec<Self>, Error> {
        let (connection, rebuilt) = schema::open(dir)?;
        moves::forget_settled(&connection)?;
        let shared = Rc::new(Shared {
            connection: RefCell::new(connection),
            rebuilt,
            in_memory: Cell::new(false),
            unreported_failure: Cell::new(false),
        });

        Ok(accounts
            .iter()
            .enumerate()
            .map(|(account, email)| Self {
                shared: Rc::clone(&shared),
                account,
                email: email.to_lowercase().into(),
            })
            .collect())
    }

    /// Whether opening replaced a corrupt or outdated Cache.
    pub fn rebuilt(&self) -> bool {
        self.shared.rebuilt
    }

    /// Whether a write to disk failed since the last call. True at most
    /// once a session, as the Cache stays in memory from then on.
    pub fn take_write_failure(&self) -> bool {
        self.shared.unreported_failure.replace(false)
    }

    pub(super) fn connection(&self) -> Ref<'_, Connection> {
        self.shared.connection.borrow()
    }

    pub(super) fn email(&self) -> &str {
        &self.email
    }

    /// The newest `limit` rows of `folder`, all of them when `None`; `None`
    /// before its first resync has written it.
    pub fn inbox(&self, folder: &str, limit: Option<usize>) -> Result<Option<Inbox>, Error> {
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

        // SQLite reads a negative LIMIT as no limit.
        let limit = limit.map_or(-1, |n| i64::try_from(n).unwrap_or(i64::MAX));
        let mut statement = connection.prepare_cached(
            "SELECT uid, body_key, received, received_offset, unread, sender, subject,
                    coalesce(snippet, ''), thread, from_me, answered
             FROM message WHERE account = ?3 AND mailbox = ?1
             ORDER BY received DESC, uid DESC LIMIT ?2",
        )?;
        let name: Arc<str> = folder.into();
        let messages = statement
            .query_map(params![folder, limit, self.email()], |row| {
                Ok(Message {
                    folder: Some(Arc::clone(&name)),
                    ..self.message(row)?
                })
            })?
            .collect::<Result<_, _>>()?;
        // Only those whose body is cached: that body comes with the Thread alone.
        let mut statement = connection.prepare_cached(
            "SELECT 0, t.key, t.received, t.received_offset, t.unread, t.sender, t.subject,
                    t.snippet, t.thread, t.from_me, t.answered
             FROM thread_message t JOIN body ON body.account = t.account AND body.key = t.key
             WHERE t.account = ?2
               AND t.thread IN (SELECT thread FROM message WHERE account = ?2 AND mailbox = ?1)
               AND t.key NOT IN
                   (SELECT body_key FROM message WHERE account = ?2 AND mailbox = ?1)
             ORDER BY t.received, t.key",
        )?;
        let elsewhere = statement
            .query_map(params![folder, self.email()], |row| self.message(row))?
            .collect::<Result<_, _>>()?;
        Ok(Some(Inbox {
            messages,
            elsewhere,
        }))
    }

    /// Every cached Folder's Messages but Trash's and Spam's, newest first,
    /// each once: the copy in `shown` wins, else the first in Folder order.
    /// Their Threads' Messages found elsewhere come as in `inbox`.
    pub fn everywhere(&self, shown: Option<&str>) -> Result<Inbox, Error> {
        let mut folders = self.folders()?;
        folders.retain(|f| !matches!(f.role, Some(Role::Trash | Role::Spam)));
        folders.sort_by_key(|f| Some(&*f.name) != shown);
        let connection = self.connection();
        let mut statement = connection.prepare_cached(
            "SELECT uid, body_key, received, received_offset, unread, sender, subject,
                    coalesce(snippet, ''), thread, from_me, answered
             FROM message WHERE account = ?2 AND mailbox = ?1",
        )?;
        let (mut messages, mut keys) = (Vec::new(), HashSet::new());
        for folder in &folders {
            let rows = statement.query_map(params![&*folder.name, self.email()], |row| {
                self.message(row)
            })?;
            for row in rows {
                let row = row?;
                if keys.insert(row.body_key) {
                    messages.push(Message {
                        folder: Some(Arc::clone(&folder.name)),
                        ..row
                    });
                }
            }
        }

        messages.sort_unstable_by(|a, b| (b.received, b.uid).cmp(&(a.received, a.uid)));
        let threads: HashSet<u64> = messages.iter().filter_map(|m| m.thread).collect();
        let mut statement = connection.prepare_cached(
            "SELECT 0, t.key, t.received, t.received_offset, t.unread, t.sender, t.subject,
                    t.snippet, t.thread, t.from_me, t.answered
             FROM thread_message t JOIN body ON body.account = t.account AND body.key = t.key
             WHERE t.account = ?1
             ORDER BY t.received, t.key",
        )?;
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

    /// Replaces what is cached of Gmail's Thread `id` from other Folders
    /// with `messages`, storing `bodies` by key first, in one transaction.
    pub fn write_thread(
        &self,
        id: u64,
        messages: &[Message],
        bodies: &[(i64, Vec<u8>)],
    ) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            store_bodies(&transaction, self.email(), bodies)?;
            let id = id.cast_signed();
            transaction.execute(
                "DELETE FROM thread_message WHERE account = ?2 AND thread = ?1",
                params![id, self.email()],
            )?;
            let mut insert = transaction.prepare_cached(
                "INSERT OR REPLACE INTO thread_message
                 (key, thread, received, received_offset, unread, sender, subject, from_me,
                  snippet, answered, account)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for m in messages {
                insert.execute(params![
                    m.body_key,
                    id,
                    m.received.timestamp(),
                    m.received.offset().local_minus_utc(),
                    m.unread,
                    m.sender,
                    m.subject,
                    m.from_me,
                    m.snippet,
                    m.answered,
                    self.email(),
                ])?;
            }

            drop(insert);
            evict(&transaction)?;
            transaction.commit()
        })
    }

    pub fn has_body(&self, key: i64) -> Result<bool, Error> {
        let found = self
            .connection()
            .prepare_cached("SELECT 1 FROM body WHERE account = ?2 AND key = ?1")?
            .query_row(params![key, self.email()], |_| Ok(()))
            .optional()?;
        Ok(found.is_some())
    }

    /// Every Folder listed or shown so far, in the Folder Picker's order.
    pub fn folders(&self) -> Result<Vec<Folder>, Error> {
        let connection = self.connection();
        let mut statement = connection
            .prepare_cached("SELECT name, role, unread FROM mailbox WHERE account = ?1")?;
        let mut folders = statement
            .query_map([self.email()], |row| {
                let role: Option<String> = row.get(1)?;
                let folder = Folder::new(
                    self.account,
                    &row.get::<_, String>(0)?,
                    role.and_then(|r| Role::from_label(&r)),
                );
                Ok(Folder {
                    unread: row.get(2)?,
                    ..folder
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        folders.sort_by(Folder::order);
        Ok(folders)
    }

    /// The Folder named `name` as last listed; without a role when it never was.
    pub fn folder(&self, name: &str) -> Result<Folder, Error> {
        let listed = self.folders()?.into_iter().find(|f| *f.name == *name);
        Ok(listed.unwrap_or_else(|| Folder::new(self.account, name, None)))
    }

    /// Writes what `LIST` answered: a Folder not in `listed` leaves the
    /// Cache with its rows, and the rest keep what they had.
    pub fn list_folders(&self, listed: &[Folder]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let cached: Vec<String> = transaction
                .prepare_cached("SELECT name FROM mailbox WHERE account = ?1")?
                .query_map([self.email()], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            let mut upsert = transaction.prepare_cached(
                "INSERT INTO mailbox (account, name, role) VALUES (?3, ?1, ?2)
                 ON CONFLICT (account, name) DO UPDATE SET role = excluded.role",
            )?;
            for folder in listed {
                upsert.execute(params![
                    &*folder.name,
                    folder.role.map(Role::label),
                    self.email()
                ])?;
            }

            for gone in cached
                .iter()
                .filter(|c| !listed.iter().any(|f| *f.name == **c))
            {
                forget(&transaction, (self.email(), gone))?;
            }

            drop(upsert);
            transaction.commit()
        })
    }

    /// The Unread count `STATUS` gave for `folder`; one no longer listed is ignored.
    pub fn write_unread(&self, folder: &str, unread: usize) -> Result<(), Error> {
        self.write(|connection| {
            connection
                .prepare_cached("UPDATE mailbox SET unread = ?2 WHERE account = ?3 AND name = ?1")?
                .execute(params![folder, unread, self.email()])
                .map(drop)
        })
    }

    /// Drops `folder` and its rows, as for a Folder the server no longer has.
    pub fn remove_folder(&self, folder: &str) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            forget(&transaction, (self.email(), folder))?;
            transaction.commit()
        })
    }

    /// Sets Unread on each `(folder, uid)` and on Gmail's copies of it in
    /// other Folders, adjusting their Unread counts, and queues the change
    /// for the server. Returns whether any row changed.
    pub fn mark(&self, messages: &[(Arc<str>, u32)], unread: bool) -> Result<bool, Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut changed = false;
            for (folder, uid) in messages {
                changed |= mark(&transaction, (self.email(), folder, *uid), unread)?;
            }

            transaction.commit()?;
            Ok(changed)
        })
    }

    /// Sets `\Answered` on `uid` of `folder` and on Gmail's copies of it,
    /// and queues it for the server.
    pub fn answer(&self, folder: &str, uid: u32) -> Result<(), Error> {
        let account = self.email();
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let key: Option<i64> = transaction
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

            queue_flag(&transaction, (account, folder, uid), Flag::Answered, true)?;
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
    pub fn uids(&self, folder: &str) -> Result<Vec<u32>, Error> {
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

    pub fn oldest_uid(&self, folder: &str) -> Result<Option<u32>, Error> {
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

    /// Up to `limit` Messages of `folder` still without a Snippet, newest
    /// first, and the part each one's Snippet is cut from; only UIDs below
    /// `below` when given.
    pub fn pending_snippets(
        &self,
        folder: &str,
        limit: usize,
        below: Option<u32>,
    ) -> Result<Vec<(u32, TextPart)>, Error> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let connection = self.connection();
        let mut statement = connection.prepare_cached(
            "SELECT uid, text_section, text_encoding, text_charset, text_html FROM message
             WHERE account = ?4 AND mailbox = ?3 AND snippet IS NULL
               AND (?2 IS NULL OR uid < ?2)
             ORDER BY received DESC, uid DESC LIMIT ?1",
        )?;
        // A Message with no text part was written with an empty Snippet.
        let pending = statement
            .query_map(params![limit, below, folder, self.email()], |row| {
                Ok((row.get(0)?, text_part(row, 1)?))
            })?
            .filter_map(|row| row.map(|(uid, part)| part.map(|p| (uid, p))).transpose())
            .collect::<Result<_, _>>()?;
        Ok(pending)
    }

    /// Writes one batch of Snippets in one transaction; a UID no longer
    /// cached is ignored.
    pub fn write_snippets(&self, folder: &str, snippets: &[(u32, String)]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut update = transaction.prepare_cached(
                "UPDATE message SET snippet = ?2 WHERE account = ?4 AND mailbox = ?3 AND uid = ?1",
            )?;
            for (uid, snippet) in snippets {
                update.execute(params![uid, snippet, folder, self.email()])?;
            }

            drop(update);
            transaction.commit()
        })
    }

    /// The body cached under `key`, marked as just used; `None` until it is
    /// fetched.
    pub fn body(&self, key: i64) -> Result<Option<Vec<u8>>, Error> {
        let found = self
            .connection()
            .prepare_cached("SELECT raw FROM body WHERE account = ?2 AND key = ?1")?
            .query_row(params![key, self.email()], |row| row.get(0))
            .optional()?;
        let Some(body) = found else {
            return Ok(None);
        };

        self.write(|connection| {
            connection
                .prepare_cached(
                    "UPDATE body SET used = (SELECT max(used) + 1 FROM body)
                     WHERE account = ?2 AND key = ?1",
                )?
                .execute(params![key, self.email()])
                .map(drop)
        })?;
        Ok(Some(body))
    }

    /// Stores each `(uid, raw)` under its Message's body key in one
    /// transaction, then evicts past `BODY_BYTES`. A UID no longer cached is
    /// ignored.
    pub fn write_bodies(&self, folder: &str, bodies: &[(u32, Vec<u8>)]) -> Result<(), Error> {
        self.write(|connection| {
            let transaction = connection.unchecked_transaction()?;
            let mut insert = transaction.prepare_cached(
                "INSERT OR REPLACE INTO body (account, key, raw, size, used)
                 SELECT account, body_key, ?2, ?3, (SELECT coalesce(max(used), 0) + 1 FROM body)
                 FROM message WHERE account = ?5 AND mailbox = ?4 AND uid = ?1",
            )?;
            for (uid, raw) in bodies {
                insert.execute(params![uid, raw, raw.len(), folder, self.email()])?;
            }

            drop(insert);
            evict(&transaction)?;
            transaction.commit()
        })
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

    /// Runs `write` on disk; when that fails, copies what the disk holds
    /// into memory and runs it there. `write` must leave nothing behind
    /// when it fails, as a transaction does.
    pub(super) fn write<T>(
        &self,
        write: impl Fn(&Connection) -> rusqlite::Result<T>,
    ) -> Result<T, Error> {
        let shared = &self.shared;
        let error = match write(&shared.connection.borrow()) {
            Ok(done) => return Ok(done),
            Err(error) if shared.in_memory.get() => return Err(error.into()),
            Err(error) => error,
        };

        tracing::error!("cache write failed, keeping the Cache in memory: {error}");
        let mut memory = Connection::open_in_memory()?;
        let copied = Backup::new(&shared.connection.borrow(), &mut memory)?.step(-1)?;
        if copied != StepResult::Done {
            return Err(error.into());
        }

        *shared.connection.borrow_mut() = memory;
        shared.in_memory.set(true);
        shared.unreported_failure.set(true);
        Ok(write(&shared.connection.borrow())?)
    }

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
            thread: row.get::<_, Option<i64>>(8)?.map(i64::cast_unsigned),
            from_me: row.get(9)?,
            answered: row.get(10)?,
            folder: None,
            account: self.account,
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

    let state = &changes.state;
    transaction.execute(
        "INSERT INTO mailbox (account, name, uid_validity, uid_next, highest_modseq, unread)
         VALUES (?6, ?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (account, name) DO UPDATE SET uid_validity = excluded.uid_validity,
             uid_next = excluded.uid_next, highest_modseq = excluded.highest_modseq,
             unread = excluded.unread",
        params![
            folder,
            state.uid_validity,
            state.uid_next,
            state.highest_modseq,
            changes.unread,
            account,
        ],
    )?;
    drop((delete, seen, answered));
    transaction.commit()
}

/// Drops `folder` with its rows and its queued changes.
fn forget(connection: &Connection, (account, folder): (&str, &str)) -> rusqlite::Result<()> {
    for table in ["message", "flag_change", "moved"] {
        connection.execute(
            &format!("DELETE FROM {table} WHERE account = ?2 AND mailbox = ?1"),
            params![folder, account],
        )?;
    }

    connection.execute(
        "DELETE FROM mailbox WHERE account = ?2 AND name = ?1",
        params![folder, account],
    )?;
    Ok(())
}

/// One Message of `Cache::mark`; false when it is not cached or already so.
fn mark(
    connection: &Connection,
    (account, folder, uid): (&str, &str, u32),
    unread: bool,
) -> rusqlite::Result<bool> {
    let key: Option<i64> = connection
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
    queue_flag(connection, (account, folder, uid), Flag::Seen, !unread)?;
    Ok(true)
}

/// Queues `flag` on `uid` of `folder` for the server, under the UID the
/// server holds it by after an Undo.
fn queue_flag(
    connection: &Connection,
    (account, folder, uid): (&str, &str, u32),
    flag: Flag,
    added: bool,
) -> rusqlite::Result<()> {
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

fn store_bodies(
    connection: &Connection,
    account: &str,
    bodies: &[(i64, Vec<u8>)],
) -> rusqlite::Result<()> {
    let mut insert = connection.prepare_cached(
        "INSERT OR REPLACE INTO body (account, key, raw, size, used)
         VALUES (?4, ?1, ?2, ?3, (SELECT coalesce(max(used), 0) + 1 FROM body))",
    )?;
    for (key, raw) in bodies {
        insert.execute(params![key, raw, raw.len(), account])?;
    }

    Ok(())
}

/// Drops the least recently used bodies past `BODY_BYTES`, of every Account.
fn evict(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM body WHERE rowid IN (
             SELECT rowid FROM (
                 SELECT rowid, sum(size) OVER (ORDER BY used DESC) AS kept FROM body
             ) WHERE kept > ?1
         )",
        [BODY_BYTES],
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
          answered, account)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
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
            m.thread.map(u64::cast_signed),
            m.from_me,
            m.answered,
            account,
        ])?;
    }

    drop(insert);
    correspondence::learn(connection, (account, folder), rows)
}

/// The text part in the four columns from `first`: section, encoding,
/// charset and html; `None` when the section is NULL.
fn text_part(row: &Row<'_>, first: usize) -> rusqlite::Result<Option<TextPart>> {
    let Some(section) = row.get::<_, Option<String>>(first)? else {
        return Ok(None);
    };

    let encoding: String = row.get(first + 1)?;
    Ok(Some(TextPart {
        section,
        encoding: TransferEncoding::from_label(&encoding),
        charset: row.get(first + 2)?,
        html: row.get(first + 3)?,
    }))
}
