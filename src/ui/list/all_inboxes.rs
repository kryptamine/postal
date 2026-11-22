use std::cmp::Reverse;

use crate::{
    cache::{self, Cache},
    domain::folder_name::FolderName,
    domain::per_account::{AccountId, PerAccount},
    domain::thread_row::ThreadRow,
    ui::list::inbox_view::InboxView,
};

/// Every Account's Inbox as one.
pub struct Inboxes {
    /// Newest first.
    pub rows: Vec<ThreadRow>,
    /// The cache edge in seconds that ends the rows, with its Account.
    pub edge: Option<(i64, AccountId)>,
}

/// Every Account's Inbox, its newest `limit` Threads or all, ended by the
/// latest cache edge of an Account that may hold older mail (ADR 0014);
/// `None` before any Inbox is resynced. Reads into `rows`.
pub fn read(
    caches: &PerAccount<Cache>,
    view: &InboxView,
    limit: Option<usize>,
    mut rows: Vec<ThreadRow>,
) -> Result<Option<Inboxes>, cache::Error> {
    let (mut edge, mut synced) = (None, false);
    for (account, cache) in caches.iter() {
        let Some(held) = cache.thread_rows(FolderName::inbox(), limit, &mut rows)? else {
            continue;
        };

        synced = true;
        let bottomed = view.bottomed(cache, account, (FolderName::inbox(), held))?;
        let arrival = cache.oldest_arrival(FolderName::inbox())?;
        if !bottomed && arrival > edge.map(|(at, _)| at) {
            edge = arrival.map(|at| (at, account));
        }
    }

    if !synced {
        return Ok(None);
    }

    let edge = edge.map(|(at, account)| (at.timestamp(), account));
    // A Thread's newest Message decides whether any of it is shown.
    if let Some((edge, _)) = edge {
        rows.retain(|row| row.newest.0 >= edge);
    }

    // Stable: of one arrival and UID, the first Account's goes first.
    rows.sort_by_key(|row| Reverse(row.newest));
    rows.truncate(limit.unwrap_or(usize::MAX));
    Ok(Some(Inboxes { rows, edge }))
}
