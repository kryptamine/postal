use std::ops::RangeInclusive;

use chrono::{DateTime, FixedOffset, Offset, Utc};
use rusqlite::{Row, Rows, params, types::Type};

use super::{Cache, Error};
use crate::{
    domain::folder_name::FolderName, domain::message::Message, domain::pool::Pool,
    domain::thread::Key,
};

// Literals, not `format!`: refilling the window allocates nothing (ADR
// 0001). `SQLite` takes the local offset from the C library, where
// chrono's `Local` would reread the time zone, allocating.
macro_rules! columns {
    ($t:literal) => {
        concat!(
            $t,
            "received, unixepoch(",
            $t,
            "received, 'unixepoch', 'localtime') - ",
            $t,
            "received, ",
            $t,
            "unread, ",
            $t,
            "sender, ",
            $t,
            "subject, "
        )
    };
}

/// The window's Threads, as column `$by` names them: those with a Message
/// arrived within `?3..=?4`.
macro_rules! threads {
    ($by:literal) => {
        concat!(
            "SELECT ",
            $by,
            " FROM message
             WHERE account = ?1 AND mailbox = ?2 AND received BETWEEN ?3 AND ?4"
        )
    };
}

// Without the index named, the planner walks the whole Folder by arrival
// to skip a sort; `+received` keeps `since`, mostly no bound, off it too.
const HERE: &str = concat!(
    "SELECT uid, body_key, ",
    columns!(""),
    "coalesce(snippet, ''), thread, from_me, answered, from_line, to_line, attachments, root
     FROM message INDEXED BY message_by_thread
     WHERE account = ?1 AND mailbox = ?2 AND +received >= ?5 AND thread IN (",
    threads!("thread"),
    ")
     UNION ALL
     SELECT uid, body_key, ",
    columns!(""),
    "coalesce(snippet, ''), thread, from_me, answered, from_line, to_line, attachments, root
     FROM message INDEXED BY message_by_root
     WHERE account = ?1 AND mailbox = ?2 AND +received >= ?5 AND thread IS NULL AND root IN (",
    threads!("root"),
    ")
     UNION ALL
     SELECT uid, body_key, ",
    columns!(""),
    "coalesce(snippet, ''), thread, from_me, answered, from_line, to_line, attachments, root
     FROM message
     WHERE account = ?1 AND mailbox = ?2 AND thread IS NULL AND root IS NULL
       AND received BETWEEN ?3 AND ?4 AND +received >= ?5
     ORDER BY received DESC, uid DESC"
);

const ELSEWHERE: &str = concat!(
    "SELECT 0, t.key, ",
    columns!("t."),
    "t.snippet, t.thread, t.from_me, t.answered, t.from_line, t.to_line, t.attachments, NULL
     FROM thread_message t JOIN body ON body.account = t.account AND body.key = t.key
     WHERE t.account = ?1 AND t.thread IN (",
    threads!("thread"),
    ")
       AND NOT EXISTS (SELECT 1 FROM message m INDEXED BY message_by_thread
                       WHERE m.account = ?1 AND m.mailbox = ?2
                         AND m.thread = t.thread AND m.body_key = t.key)
     ORDER BY t.received, t.key"
);

/// Which Messages a window reads, by arrival in seconds.
pub struct Arrivals {
    /// Its Threads are those with a Message arrived within it.
    pub within: RangeInclusive<i64>,
    /// Of the shown Folder, those arrived before it stay out.
    pub since: i64,
}

impl Cache {
    /// Fills `here` with the Messages of `folder` of the Threads `wanted`
    /// keeps among those `arrivals` names, newest first, and `elsewhere`
    /// with those Threads' Messages in other Folders whose body is cached,
    /// oldest first. Allocates only to grow the slots (ADR 0001).
    pub fn window(
        &self,
        folder: &FolderName,
        arrivals: &Arrivals,
        wanted: impl Fn(Key) -> bool,
        (here, elsewhere): (&mut Pool<Message>, &mut Pool<Message>),
    ) -> Result<(), Error> {
        let connection = self.connection();
        let (from, to, since) = (
            *arrivals.within.start(),
            *arrivals.within.end(),
            arrivals.since,
        );
        // Not `prepare_cached`: it allocates its key on every call.
        let mut statement = connection.prepare(HERE)?;
        let rows = statement.query(params![self.email(), folder, from, to, since])?;
        self.fill(rows, Some(folder), &wanted, here)?;
        let mut statement = connection.prepare(ELSEWHERE)?;
        let rows = statement.query(params![self.email(), folder, from, to])?;
        self.fill(rows, None, &wanted, elsewhere)?;
        Ok(())
    }

    /// Reads each of `rows` into the next slot of `into`, given back unless
    /// `wanted` keeps its Thread.
    fn fill(
        &self,
        mut rows: Rows<'_>,
        folder: Option<&FolderName>,
        wanted: &impl Fn(Key) -> bool,
        into: &mut Pool<Message>,
    ) -> rusqlite::Result<()> {
        while let Some(row) = rows.next()? {
            let message = into.next(|| Message::blank(self.account()));
            read(row, message)?;
            message.folder = folder.cloned();
            message.account = self.account();
            if !wanted(Key::of(message)) {
                into.pop();
            }
        }

        Ok(())
    }
}

/// A row of `HERE` or `ELSEWHERE` into `message`, its strings' buffers reused.
pub(super) fn read(row: &Row<'_>, message: &mut Message) -> rusqlite::Result<()> {
    let offset = FixedOffset::east_opt(row.get(3)?).unwrap_or_else(|| Utc.fix());
    message.received = DateTime::from_timestamp(row.get(2)?, 0)
        .unwrap_or_default()
        .with_timezone(&offset);
    message.uid = row.get(0)?;
    message.body_key = row.get(1)?;
    message.unread = row.get(4)?;
    text(row, 5, &mut message.sender)?;
    text(row, 6, &mut message.subject)?;
    text(row, 7, &mut message.snippet)?;
    message.thread = row.get(8)?;
    message.from_me = row.get(9)?;
    message.answered = row.get(10)?;
    text(row, 11, &mut message.from)?;
    text(row, 12, &mut message.to)?;
    text(row, 13, message.attachments.stored_mut())?;
    message.root = row.get(14)?;
    Ok(())
}

fn text(row: &Row<'_>, index: usize, into: &mut String) -> rusqlite::Result<()> {
    let value = row.get_ref(index)?.as_str().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })?;
    into.clear();
    into.push_str(value);
    Ok(())
}
