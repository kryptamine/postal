//! What a Search reads from the Cache: see
//! docs/adr/0023-search-finds-candidates-in-fts5-and-ranks-them-with-nucleo.md.

use std::{
    collections::{HashMap, hash_map::Entry},
    fmt::Write,
};

use rusqlite::{Row, params, params_from_iter, types::Type};

use super::{
    Cache, Error,
    scope::{self, Scope},
    window,
};
use crate::{
    domain::body_key::BodyKey,
    domain::message::Message,
    domain::per_account::{AccountId, PerAccount},
    domain::pool::Pool,
    domain::thread::Key,
};

/// The columns `window::read` takes, of a `message` row, at the local
/// offset: the cards show local dates. Literals, so a window allocates
/// nothing.
macro_rules! message_row {
    () => {
        "uid, body_key, received,
         unixepoch(received, 'unixepoch', 'localtime') - received, unread, sender, subject,
         coalesce(snippet, ''), thread, from_me, answered, from_line, to_line, attachments, root"
    };
}

/// The rows of one Message's Threads, `?1`, of Gmail Threads, `?3`, and of
/// Threads by `References`, `?4`, as JSON arrays. Without the indexes
/// named, the planner scans the Account.
const HERE: &str = concat!(
    "SELECT ",
    message_row!(),
    ", mailbox FROM message WHERE rowid IN (SELECT value FROM json_each(?1))
     UNION ALL SELECT ",
    message_row!(),
    ", mailbox FROM message INDEXED BY message_by_thread
     WHERE account = ?2 AND thread IN (SELECT value FROM json_each(?3))
     UNION ALL SELECT ",
    message_row!(),
    ", mailbox FROM message INDEXED BY message_by_root
     WHERE account = ?2 AND root IN (SELECT value FROM json_each(?4))"
);

/// The Messages of the Gmail Threads `?2` whose body is cached, by key.
const ELSEWHERE: &str = "SELECT 0, t.key, t.received,
    unixepoch(t.received, 'unixepoch', 'localtime') - t.received, t.unread, t.sender,
    t.subject, t.snippet, t.thread, t.from_me, t.answered, t.from_line, t.to_line,
    t.attachments, NULL
    FROM thread_message t JOIN body ON body.account = t.account AND body.key = t.key
    WHERE t.account = ?1 AND t.thread IN (SELECT value FROM json_each(?2))";

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
    ", m.root FROM message m ORDER BY m.rowid DESC"
);

/// The Messages matching `?1`, newest first, with the text they are ranked by.
const MATCHING: &str = concat!(
    "SELECT m.account, m.mailbox, m.body_key, m.thread, m.rowid, ",
    ranked_text!(),
    ", m.root FROM message_text JOIN message m ON m.rowid = message_text.rowid
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
/// each, the row of the copy `scope` ranks first.
pub fn candidates(
    caches: &PerAccount<Cache>,
    scope: &Scope,
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
        let Some((account, folders)) =
            (scope.accounts.iter()).find(|(account, _)| caches[*account].email() == email)
        else {
            continue;
        };

        let Some(rank) = scope::rank(folders, text(row, 1)?) else {
            continue;
        };

        let (account, body) = (*account, row.get::<_, BodyKey>(2)?);
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
                    key: Key::new(account, row.get(3)?, row.get(6)?, body),
                    row: rowid,
                    text: text(row, 5)?.to_owned(),
                });
            }
        }
    }

    Ok(found)
}

/// Fills `here` with the Messages of `wanted`'s Threads, by key and a row,
/// in `scope`'s Folders, newest first, each once as it ranks them; `elsewhere`
/// with those only elsewhere whose body is cached, oldest first.
pub fn threads(
    caches: &PerAccount<Cache>,
    scope: &mut Scope,
    wanted: &[(Key, i64)],
    (here, elsewhere): (&mut Pool<Message>, &mut Pool<Message>),
) -> Result<(), Error> {
    scope.ranks.clear();
    for at in 0..scope.accounts.len() {
        let account = scope.accounts[at].0;
        if lists(account, wanted, scope) {
            caches[account].threads_of(scope, at, (here, elsewhere))?;
        }
    }

    // Unstable: a stable sort allocates its buffer.
    here.as_mut_slice()
        .sort_unstable_by_key(|m| std::cmp::Reverse((m.received, m.uid)));
    elsewhere
        .as_mut_slice()
        .sort_unstable_by_key(|m| (m.received, m.body_key.get()));
    Ok(())
}

impl Cache {
    /// Appends this Account's Messages of the Threads `scope`'s lists name,
    /// its Folders the `at`th of `scope`'s.
    fn threads_of(
        &self,
        scope: &mut Scope,
        at: usize,
        (here, elsewhere): (&mut Pool<Message>, &mut Pool<Message>),
    ) -> Result<(), Error> {
        let connection = self.connection();
        let (account, start) = (self.account(), here.as_slice().len());
        let folders = &scope.accounts[at].1;
        // Not `prepare_cached`: it allocates its key on every call.
        let mut statement = connection.prepare(HERE)?;
        let mut rows = statement.query(params![
            scope.lone,
            self.email(),
            scope.threads,
            scope.roots
        ])?;
        while let Some(row) = rows.next()? {
            let Some(rank) = scope::rank(folders, text(row, 15)?) else {
                continue;
            };

            let body: BodyKey = row.get(1)?;
            let read = here.as_slice()[start..]
                .iter()
                .position(|m| m.body_key == body);
            let slot = match read {
                Some(i) if rank >= scope.ranks[start + i] => continue,
                Some(i) => {
                    scope.ranks[start + i] = rank;
                    &mut here.as_mut_slice()[start + i]
                }
                None => {
                    scope.ranks.push(rank);
                    here.next(|| Message::blank(account))
                }
            };

            window::read(row, slot)?;
            slot.folder = Some(folders[rank].clone());
            slot.account = account;
        }

        let mut statement = connection.prepare(ELSEWHERE)?;
        let mut rows = statement.query(params![self.email(), scope.threads])?;
        while let Some(row) = rows.next()? {
            let body: BodyKey = row.get(1)?;
            if here.as_slice()[start..].iter().any(|m| m.body_key == body) {
                continue;
            }

            let slot = elsewhere.next(|| Message::blank(account));
            window::read(row, slot)?;
            slot.folder = None;
            slot.account = account;
        }

        Ok(())
    }
}

/// Writes into `scope`'s lists `account`'s Threads of `wanted` as JSON
/// arrays: the rows of those of one Message, the Gmail Threads and those
/// by `References`; false when it has none.
fn lists(account: AccountId, wanted: &[(Key, i64)], scope: &mut Scope) -> bool {
    let (lone, threads, roots) = (&mut scope.lone, &mut scope.threads, &mut scope.roots);
    for list in [&mut *lone, &mut *threads, &mut *roots] {
        list.clear();
        list.push('[');
    }

    // Writing to a `String` cannot fail.
    for &(key, row) in wanted {
        match key {
            Key::Gmail(of, id) if of == account => {
                write!(threads, "{},", id.get().cast_signed()).ok();
            }
            Key::Root(of, root) if of == account => {
                write!(roots, "{},", root.get().cast_signed()).ok();
            }
            Key::Lone(of, _) if of == account => {
                write!(lone, "{row},").ok();
            }
            _ => {}
        }
    }

    let any = lone.len() > 1 || threads.len() > 1 || roots.len() > 1;
    for list in [lone, threads, roots] {
        if list.ends_with(',') {
            list.pop();
        }

        list.push(']');
    }

    any
}

/// The Query as `FTS5` terms: each word a quoted prefix, all required, cut
/// as `unicode61` cuts the text, so no input reaches the query syntax.
pub(super) fn terms(query: &str) -> String {
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

fn text<'r>(row: &'r Row<'_>, index: usize) -> rusqlite::Result<&'r str> {
    row.get_ref(index)?.as_str().map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}
