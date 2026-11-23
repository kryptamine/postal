use std::{collections::HashMap, collections::HashSet, fmt::Write, ops::ControlFlow};

use chrono::{DateTime, FixedOffset};
use serde::{Serialize, Serializer};

use crate::{
    cache::Listed, cli::ids, domain::body_key::BodyKey, domain::modified_utf7, domain::thread::Key,
    render::header_date,
};

/// One Thread of `--json`, as a row of the Message List shows it; its
/// fields only grow (ADR 0017).
#[derive(Serialize)]
pub struct Row {
    thread: String,
    /// Its newest Message.
    message: String,
    account: String,
    /// Its newest Message's.
    folder: String,
    /// Each `From` once, in order of first appearance.
    senders: Vec<String>,
    /// Its oldest Message's.
    subject: String,
    /// Its newest Message's, empty until fetched.
    snippet: String,
    /// Its newest Message's, in RFC 3339 as every command writes a date.
    #[serde(serialize_with = "rfc3339")]
    date: DateTime<FixedOffset>,
    unread: bool,
    messages: usize,
}

/// The first `limit` Threads of Messages read newest first, each Message
/// once however many Folders hold it.
pub struct ThreadRows {
    rows: Vec<Row>,
    at: HashMap<Key<String>, usize>,
    seen: HashSet<(usize, BodyKey)>,
    limit: usize,
}

impl ThreadRows {
    pub fn new(limit: usize) -> Self {
        Self {
            rows: Vec::new(),
            at: HashMap::new(),
            seen: HashSet::new(),
            limit,
        }
    }

    /// Never breaks: a Thread's older Messages count however late they come.
    pub fn visit(&mut self, listed: Listed) -> ControlFlow<()> {
        let key = Key::new(listed.account, listed.thread, listed.root, listed.body_key);
        let at = match self.at.get(&key) {
            Some(&at) => at,
            None if self.rows.len() == self.limit => return ControlFlow::Continue(()),
            None => {
                self.rows.push(Row {
                    thread: ids::thread(&key),
                    message: ids::message(key.account(), listed.body_key),
                    account: key.account().clone(),
                    folder: modified_utf7::decode(&listed.folder).into_owned(),
                    senders: Vec::new(),
                    subject: String::new(),
                    snippet: listed.snippet,
                    date: listed.received,
                    unread: false,
                    messages: 0,
                });

                self.at.insert(key, self.rows.len() - 1);
                self.rows.len() - 1
            }
        };

        if self.seen.insert((at, listed.body_key)) {
            let row = &mut self.rows[at];
            row.messages += 1;
            row.unread |= listed.unread;
            row.subject = listed.subject;
            row.senders.retain(|from| *from != listed.from);
            row.senders.insert(0, listed.from);
        }

        ControlFlow::Continue(())
    }

    pub fn into_rows(self) -> Vec<Row> {
        self.rows
    }
}

fn rfc3339<S: Serializer>(date: &DateTime<FixedOffset>, to: S) -> Result<S::Ok, S::Error> {
    to.serialize_str(&date.to_rfc3339())
}

/// A line each: Unread as `*`, the date, the senders, the subject with the
/// count of Messages past one, and the Thread's ID.
pub fn table(rows: &[Row]) -> String {
    let mut text = String::new();
    for row in rows {
        let unread = if row.unread { '*' } else { ' ' };
        let count = if row.messages > 1 {
            format!(" ({})", row.messages)
        } else {
            String::new()
        };

        // Writing to a `String` cannot fail.
        writeln!(
            text,
            "{unread} {}  {}  {}{count}  {}",
            header_date::date(&row.date).as_str(),
            row.senders.join(", "),
            row.subject,
            row.thread
        )
        .ok();
    }

    text
}
