use std::{collections::HashMap, collections::HashSet, fmt::Write, ops::ControlFlow};

use serde::Serialize;

use crate::{
    cache::{Listed, ThreadOf},
    cli::ids,
    domain::body_key::BodyKey,
    render::header_date,
};

/// One Thread of `--json`, as a row of the Message List shows it; its
/// fields only grow (ADR 0017).
#[derive(Serialize)]
pub struct Row {
    pub thread: String,
    /// Its newest Message.
    pub message: String,
    pub account: String,
    /// Its newest Message's.
    pub folder: String,
    /// Each `From` once, in order of first appearance.
    pub senders: Vec<String>,
    /// Its oldest Message's.
    pub subject: String,
    /// Its newest Message's, empty until fetched.
    pub snippet: String,
    /// RFC 3339, its newest Message's.
    pub date: String,
    pub unread: bool,
    pub messages: usize,
    #[serde(skip)]
    shown_date: String,
    #[serde(skip)]
    keys: HashSet<BodyKey>,
}

/// The first `limit` Threads of Messages read newest first, each Message
/// once however many Folders hold it.
pub struct ThreadRows {
    rows: Vec<Row>,
    at: HashMap<(String, ThreadOf), usize>,
    limit: usize,
}

impl ThreadRows {
    pub fn new(limit: usize) -> Self {
        Self {
            rows: Vec::new(),
            at: HashMap::new(),
            limit,
        }
    }

    /// Never breaks: a Thread's older Messages count however late they come.
    pub fn visit(&mut self, listed: Listed) -> ControlFlow<()> {
        let key = ThreadOf::of(listed.thread, listed.root, listed.body_key);
        let found = self.at.get(&(listed.account.clone(), key)).copied();
        let row = match found {
            Some(at) => &mut self.rows[at],
            None if self.rows.len() == self.limit => return ControlFlow::Continue(()),
            None => {
                self.at
                    .insert((listed.account.clone(), key), self.rows.len());
                self.rows.push(Row {
                    thread: ids::thread(&listed.account, key),
                    message: ids::message(&listed.account, listed.body_key),
                    folder: listed.folder.clone(),
                    snippet: listed.snippet.clone(),
                    date: listed.received.to_rfc3339(),
                    shown_date: header_date::date(&listed.received).as_str().to_owned(),
                    account: listed.account.clone(),
                    senders: Vec::new(),
                    subject: String::new(),
                    unread: false,
                    messages: 0,
                    keys: HashSet::new(),
                });

                let last = self.rows.len() - 1;
                &mut self.rows[last]
            }
        };

        if row.keys.insert(listed.body_key) {
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
            row.shown_date,
            row.senders.join(", "),
            row.subject,
            row.thread
        )
        .ok();
    }

    text
}
