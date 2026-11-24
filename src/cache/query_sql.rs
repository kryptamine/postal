//! A Query as the SQL that reads its matches: see
//! docs/adr/0031-the-query-takes-operators.md.

use std::{fmt::Write, ops::ControlFlow};

use rusqlite::{Connection, Row, ToSql};

use super::Error;
use crate::domain::query::{Field, Kind, Query, Term};

/// What a word without an operator matches: `from_line` and `to_line` are
/// for the operators alone.
const ANY: &str = "{sender subject snippet recipients}";

/// A statement reading some columns of the `message m` rows matching a
/// Query in some Folders, newest first, with what it binds.
pub(super) struct Select {
    sql: String,
    /// The Folders read, a JSON array of `[account, mailbox]`.
    places: String,
    /// The words a row holds, as FTS5 takes them; empty for none.
    matching: String,
    /// The words it must not hold, when it need hold none.
    excluded: String,
    /// The rows a flag term lets in whatever their flags, a JSON array,
    /// when the Query has a flag term.
    kept: Option<String>,
}

impl Select {
    /// `columns` of the rows matching `query` in `places`, each an Account's
    /// address and a Folder; with `kept`, a flag term also lets those rows in.
    pub fn new(
        columns: &str,
        query: &Query,
        places: &[(&str, &str)],
        kept: Option<&[i64]>,
    ) -> Self {
        let (held, shunned) = (phrases(query, false), phrases(query, true));
        let (matching, excluded) = match (held.is_empty(), shunned.is_empty()) {
            (_, true) => (held.join(" AND "), String::new()),
            (true, false) => (String::new(), shunned.join(" OR ")),
            (false, false) => {
                let (held, shunned) = (held.join(" AND "), shunned.join(" OR "));
                (format!("({held}) NOT ({shunned})"), String::new())
            }
        };

        let flagged = query
            .terms
            .iter()
            .any(|t| matches!(t.kind, Kind::Unread(_)));
        let kept = kept.filter(|_| flagged).map(json);
        let (mut sql, rowid) = if matching.is_empty() {
            // By the primary key the planner sorts every row of the Folders,
            // 275 ms at 500,000 for `is:unread`; by rowid it stops at `N`.
            let sql = format!("SELECT {columns} FROM message m NOT INDEXED WHERE true");
            (sql, "m.rowid")
        } else {
            let sql = format!(
                "SELECT {columns} FROM message_text JOIN message m ON m.rowid = message_text.rowid
                 WHERE message_text MATCH :matching"
            );
            (sql, "message_text.rowid")
        };

        sql.push_str(
            " AND (m.account, m.mailbox) IN
              (SELECT value ->> 0, value ->> 1 FROM json_each(:places))",
        );
        if !excluded.is_empty() {
            sql.push_str(
                " AND m.rowid NOT IN
                  (SELECT rowid FROM message_text WHERE message_text MATCH :excluded)",
            );
        }

        for term in &query.terms {
            clause(&mut sql, rowid, term, kept.is_some());
        }

        // Writing to a `String` cannot fail.
        write!(sql, " ORDER BY {rowid} DESC").ok();
        Self {
            sql,
            places: json(places),
            matching,
            excluded,
            kept,
        }
    }

    /// Calls `each` with every row, newest first, until it breaks.
    pub fn rows(
        &self,
        connection: &Connection,
        mut each: impl FnMut(&Row<'_>) -> Result<ControlFlow<()>, Error>,
    ) -> Result<(), Error> {
        let mut params: Vec<(&str, &dyn ToSql)> = vec![(":places", &self.places)];
        for (name, value) in [(":matching", &self.matching), (":excluded", &self.excluded)] {
            if !value.is_empty() {
                params.push((name, value));
            }
        }

        if let Some(kept) = &self.kept {
            params.push((":kept", kept));
        }

        let mut statement = connection.prepare_cached(&self.sql)?;
        let mut rows = statement.query(&*params)?;
        while let Some(row) = rows.next()? {
            if each(row)?.is_break() {
                break;
            }
        }

        Ok(())
    }
}

/// The words the terms `negated`, or not, ask for, each an FTS5 quoted
/// prefix behind its column, so no input reaches the query syntax.
fn phrases(query: &Query, negated: bool) -> Vec<String> {
    let mut phrases = Vec::new();
    for term in query.terms.iter().filter(|t| t.negated == negated) {
        let Kind::Words {
            field,
            words,
            phrase,
        } = &term.kind
        else {
            continue;
        };

        let column = match field {
            Field::Any => ANY,
            Field::From => "from_line",
            Field::To => "to_line",
            Field::Subject => "subject",
        };

        let quoted = |text: &str| format!("{column} : \"{text}\"*");
        if *phrase {
            phrases.push(quoted(&words.join(" ")));
        } else {
            phrases.extend(words.iter().map(|word| quoted(word)));
        }
    }

    phrases
}

/// Appends the `AND` clause of a term on `message m` that is no words; a
/// flag's also lets in `:kept` with `keep`. A date is on the rowid, the
/// arrival second shifted left 24 bits, so FTS5 reads only that range.
fn clause(sql: &mut String, rowid: &str, term: &Term, keep: bool) {
    let shifted = |second: i64| second.saturating_mul(1 << 24);
    let condition = match term.kind {
        Kind::Unread(unread) => format!("m.unread = {}", i32::from(unread)),
        Kind::Attachment => "m.attachments <> ''".to_owned(),
        Kind::Before(second) => format!("{rowid} < {}", shifted(second)),
        Kind::After(second) => format!("{rowid} >= {}", shifted(second)),
        // Words are matched, and Folders and Accounts are the places.
        Kind::Words { .. } | Kind::Folder(_) | Kind::Account(_) => return,
    };

    let not = if term.negated { "NOT " } else { "" };
    let kept = if keep && matches!(term.kind, Kind::Unread(_)) {
        " OR m.rowid IN (SELECT value FROM json_each(:kept))"
    } else {
        ""
    };

    // Writing to a `String` cannot fail.
    write!(sql, " AND ({not}({condition}){kept})").ok();
}

fn json(value: &(impl serde::Serialize + ?Sized)) -> String {
    serde_json::to_string(value).unwrap_or_default()
}
