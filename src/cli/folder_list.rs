use std::{collections::BTreeSet, error::Error, fmt::Write};

use serde::Serialize;

use crate::{
    account::xdg, cache::Reader, cli::accounts, cli::answer::Answer, domain::folder::Role,
};

/// One Folder of `--json`, whose fields only grow (ADR 0017).
#[derive(Serialize)]
struct Row<'a> {
    account: &'a str,
    /// Decoded from modified UTF-7, as `--folder` takes it.
    name: &'a str,
    /// Its special use as `Role::label` names it, such as `Trash`.
    role: Option<&'static str>,
    /// `null` until a sync counted it.
    unread: Option<usize>,
}

/// `postal folder list`: every Account's Folders as the last sync left them.
pub fn answer() -> Result<Answer, Box<dyn Error>> {
    let emails = accounts::emails()?;
    let mut folders = Reader::open(&xdg::cache_dir()?)?.folders()?;
    folders.sort_by(|a, b| a.account.cmp(&b.account).then_with(|| a.order(b)));
    let rows: Vec<Row<'_>> = folders
        .iter()
        // An Account unlinked may leave Folders until its rows are forgotten.
        .filter(|folder| emails.contains(&folder.account))
        .map(|folder| Row {
            account: &folder.account,
            name: &folder.path,
            role: folder.role.map(Role::label),
            unread: folder.unread,
        })
        .collect();
    Ok(Answer::new(&rows, table(&rows))?)
}

/// A line each: the name, its Unread count when any, its role when the name
/// is not it; the Account heads its Folders when there are several.
fn table(rows: &[Row<'_>]) -> String {
    let accounts: BTreeSet<&str> = rows.iter().map(|row| row.account).collect();
    let mut text = String::new();
    let mut heading = None;
    // Writing to a `String` cannot fail.
    for row in rows {
        if accounts.len() > 1 && heading != Some(row.account) {
            let gap = if heading.is_some() { "\n" } else { "" };
            writeln!(text, "{gap}{}", row.account).ok();
            heading = Some(row.account);
        }

        text.push_str(row.name);
        if let Some(role) = row.role.filter(|&role| role != row.name) {
            write!(text, " ({role})").ok();
        }

        if let Some(unread) = row.unread.filter(|&n| n > 0) {
            write!(text, "  {unread} unread").ok();
        }

        text.push('\n');
    }

    text
}
