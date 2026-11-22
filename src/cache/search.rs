//! What a Search reads from the Cache: see
//! docs/adr/0023-search-finds-candidates-in-fts5-and-ranks-them-with-nucleo.md.

use std::{
    collections::{HashMap, hash_map::Entry},
    fmt::Write,
};

use chrono::{DateTime, FixedOffset, Offset, Utc};
use rusqlite::{Row, params, params_from_iter, types::Type};

use super::{Cache, Error};
use crate::{
    domain::body_key::BodyKey,
    domain::folder::Role,
    domain::folder_name::FolderName,
    domain::message::Message,
    domain::per_account::{AccountId, PerAccount},
    domain::pool::Pool,
    domain::shown::Shown,
    domain::thread::Key,
    domain::thread_id::ThreadId,
};

/// The columns `message` reads, in its order, of a `message` row, at the
/// local offset: the cards show local dates.
const MESSAGE_ROW: &str = "uid, body_key, received,
    unixepoch(received, 'unixepoch', 'localtime') - received, unread, sender, subject,
    coalesce(snippet, ''), thread, from_me, answered, from_line, to_line";

/// `MESSAGE_ROW` of a `thread_message` row `t`, which has no UID.
const THREAD_ROW: &str = "0, t.key, t.received,
    unixepoch(t.received, 'unixepoch', 'localtime') - t.received, t.unread, t.sender,
    t.subject, t.snippet, t.thread, t.from_me, t.answered, t.from_line, t.to_line";

/// What a Query is ranked by: the sender, subject and Snippet and, of a
/// Message the Account sent, its recipients, a newline apart.
macro_rules! ranked_text {
    () => {
        "m.sender || char(10) || m.subject || char(10)
         || coalesce(m.snippet, '') || iif(m.from_me, char(10) || m.to_line, '')"
    };
}

/// Every Message, newest first, with the text it is ranked by.
const CANDIDATES: &str = concat!(
    "SELECT m.account, m.mailbox, m.body_key, m.thread, m.rowid, ",
    ranked_text!(),
    " FROM message m ORDER BY m.rowid DESC"
);

/// The Messages matching `?1`, newest first, with the text they are ranked by.
const MATCHING: &str = concat!(
    "SELECT m.account, m.mailbox, m.body_key, m.thread, m.rowid, ",
    ranked_text!(),
    " FROM message_text JOIN message m ON m.rowid = message_text.rowid
     WHERE message_text MATCH ?1 ORDER BY message_text.rowid DESC"
);

/// A Message whose words start with the Query's.
pub struct Candidate {
    pub key: Key,
    /// Its `message` row, by which a Thread of one Message is read again.
    pub row: i64,
    pub text: String,
}

/// The newest candidates, each Message once; whether more matched.
pub struct Candidates {
    pub found: Vec<Candidate>,
    pub more: bool,
}

/// The newest `limit` Messages of every Folder but Trash and Spam whose
/// words start with each word of `query`, every Message for no words; of
/// each, the row of its copy in `shown`, else the first in Folder order.
pub fn candidates(
    caches: &PerAccount<Cache>,
    shown: &Shown,
    query: &str,
    limit: usize,
) -> Result<Candidates, Error> {
    let mut found = Candidates {
        found: Vec::new(),
        more: false,
    };
    // Every Account's rows sit in the one store all handles share.
    let Some(cache) = caches.values().next() else {
        return Ok(found);
    };

    let accounts = searched(caches, |account| shown.of(account))?;
    let terms = terms(query);
    let sql = if terms.is_empty() {
        CANDIDATES
    } else {
        MATCHING
    };

    let connection = cache.connection();
    let mut statement = connection.prepare_cached(sql)?;
    let mut rows = statement.query(params_from_iter((!terms.is_empty()).then_some(&terms)))?;
    // Each Message's best copy so far, and where it is among the found.
    let mut seen: HashMap<(AccountId, BodyKey), (usize, Option<usize>)> = HashMap::new();
    // The arrival second of the first Message past `limit`: the copies of
    // one Message arrived together, so a better one may follow in it.
    let mut cutoff = None;
    while let Some(row) = rows.next()? {
        let rowid: i64 = row.get(4)?;
        if cutoff.is_some_and(|second| rowid >> 24 < second) {
            break;
        }

        let email = text(row, 0)?;
        let Some(searched) = accounts.iter().find(|a| a.email == email) else {
            continue;
        };

        let Some(&rank) = searched.folders.get(text(row, 1)?) else {
            continue;
        };

        let (account, body) = (searched.account, row.get::<_, BodyKey>(2)?);
        match seen.entry((account, body)) {
            Entry::Occupied(mut copy) => {
                if let (true, Some(at)) = (rank < copy.get().0, copy.get().1) {
                    found.found[at].row = rowid;
                    copy.get_mut().0 = rank;
                }
            }
            Entry::Vacant(new) if found.found.len() == limit => {
                new.insert((rank, None));
                found.more = true;
                cutoff.get_or_insert(rowid >> 24);
            }
            Entry::Vacant(new) => {
                new.insert((rank, Some(found.found.len())));
                found.found.push(Candidate {
                    key: row
                        .get::<_, Option<ThreadId>>(3)?
                        .map_or(Key::Lone(account, body), |id| Key::Gmail(account, id)),
                    row: rowid,
                    text: text(row, 5)?.to_owned(),
                });
            }
        }
    }

    Ok(found)
}

/// Fills `here` with the Messages of `wanted`'s Threads, by key and a row,
/// in every Folder but Trash and Spam, newest first, once as ADR 0010 picks;
/// `elsewhere` with those only elsewhere whose body is cached, oldest first.
pub fn threads(
    caches: &PerAccount<Cache>,
    shown: &Shown,
    wanted: &[(Key, i64)],
    (here, elsewhere): (&mut Pool<Message>, &mut Pool<Message>),
) -> Result<(), Error> {
    for searched in &searched(caches, |account| shown.of(account))? {
        caches[searched.account].threads_of(searched, wanted, (here, elsewhere))?;
    }

    here.as_mut_slice()
        .sort_by(|a, b| (b.received, b.uid).cmp(&(a.received, a.uid)));
    elsewhere.as_mut_slice().sort_by_key(|m| m.received);
    Ok(())
}

/// One Account as a Search reads it: each Folder searched by name, ranked
/// by which copy of a Message wins, lowest first.
struct Searched<'a> {
    account: AccountId,
    email: &'a str,
    folders: HashMap<String, usize>,
}

/// Every Account's Folders but Trash and Spam, the one `shown` names for
/// it first, then in Folder order.
fn searched<'a, 's>(
    caches: &'a PerAccount<Cache>,
    shown: impl Fn(AccountId) -> Option<&'s FolderName>,
) -> Result<Vec<Searched<'a>>, Error> {
    let mut accounts = Vec::new();
    for (account, cache) in caches.iter() {
        let mut folders = cache.folders()?;
        folders.retain(|f| !matches!(f.role, Some(Role::Trash | Role::Spam)));
        folders.sort_by_key(|f| Some(&f.name) != shown(account));
        accounts.push(Searched {
            account,
            email: cache.email(),
            folders: (folders.iter().enumerate())
                .map(|(rank, f)| (f.name.as_str().to_owned(), rank))
                .collect(),
        });
    }

    Ok(accounts)
}

impl Cache {
    /// Appends this Account's Messages of the Threads `wanted` names.
    fn threads_of(
        &self,
        searched: &Searched<'_>,
        wanted: &[(Key, i64)],
        (here, elsewhere): (&mut Pool<Message>, &mut Pool<Message>),
    ) -> Result<(), Error> {
        let Some((lone, threads)) = lists(searched.account, wanted) else {
            return Ok(());
        };

        let connection = self.connection();
        // Without the index named, the planner scans the Account by arrival.
        let mut statement = connection.prepare_cached(&format!(
            "SELECT {MESSAGE_ROW}, mailbox FROM message
             WHERE rowid IN (SELECT value FROM json_each(?1))
             UNION ALL
             SELECT {MESSAGE_ROW}, mailbox FROM message INDEXED BY message_by_thread
             WHERE account = ?2 AND thread IN (SELECT value FROM json_each(?3))"
        ))?;
        let mut rows = statement.query(params![lone, self.email(), threads])?;
        let mut kept: HashMap<BodyKey, (usize, Message)> = HashMap::new();
        while let Some(row) = rows.next()? {
            let Some(&rank) = searched.folders.get(text(row, 13)?) else {
                continue;
            };

            let message = message(row, self.account())?;
            match kept.entry(message.body_key) {
                Entry::Occupied(copy) if copy.get().0 <= rank => {}
                Entry::Occupied(mut copy) => {
                    copy.insert((rank, folder(row, message)?));
                }
                Entry::Vacant(new) => {
                    new.insert((rank, folder(row, message)?));
                }
            }
        }

        let mut statement = connection.prepare_cached(&format!(
            "SELECT {THREAD_ROW}
             FROM thread_message t JOIN body ON body.account = t.account AND body.key = t.key
             WHERE t.account = ?1 AND t.thread IN (SELECT value FROM json_each(?2))"
        ))?;
        let rows = statement.query_map(params![self.email(), threads], |row| {
            message(row, self.account())
        })?;
        for row in rows {
            let row = row?;
            if !kept.contains_key(&row.body_key) {
                *elsewhere.next(|| Message::blank(self.account())) = row;
            }
        }

        for (_, message) in kept.into_values() {
            *here.next(|| Message::blank(self.account())) = message;
        }

        Ok(())
    }
}

/// `account`'s Threads of `wanted` as JSON arrays: the rows of those of
/// one Message, and the Gmail Threads; `None` for none of its own.
fn lists(account: AccountId, wanted: &[(Key, i64)]) -> Option<(String, String)> {
    let (mut lone, mut threads) = (Vec::new(), Vec::new());
    for &(key, row) in wanted {
        match key {
            Key::Gmail(of, id) if of == account => threads.push(id.get().cast_signed()),
            Key::Lone(of, _) if of == account => lone.push(row),
            _ => {}
        }
    }

    let json = |ids: &[i64]| {
        let mut json = String::from("[");
        for (n, id) in ids.iter().enumerate() {
            let gap = if n == 0 { "" } else { "," };
            // Writing to a `String` cannot fail.
            write!(json, "{gap}{id}").ok();
        }

        json + "]"
    };

    (!lone.is_empty() || !threads.is_empty()).then(|| (json(&lone), json(&threads)))
}

/// The Query as `FTS5` terms: each word a quoted prefix, all required, cut
/// as `unicode61` cuts the text, so no input reaches the query syntax.
fn terms(query: &str) -> String {
    let mut terms = String::new();
    for word in query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
    {
        let gap = if terms.is_empty() { "" } else { " " };
        // Writing to a `String` cannot fail.
        write!(terms, "{gap}\"{word}\"*").ok();
    }

    terms
}

/// `message` with the Folder its row names last.
fn folder(row: &Row<'_>, message: Message) -> rusqlite::Result<Message> {
    Ok(Message {
        folder: Some(row.get(13)?),
        ..message
    })
}

fn text<'r>(row: &'r Row<'_>, index: usize) -> rusqlite::Result<&'r str> {
    row.get_ref(index)?.as_str().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}

/// A row of `MESSAGE_ROW` or `THREAD_ROW` as a Message of `account`.
fn message(row: &Row<'_>, account: AccountId) -> rusqlite::Result<Message> {
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
        account,
    })
}
