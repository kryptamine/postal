use crate::{
    cache::{self, Cache},
    folder::INBOX,
    inbox::Inbox,
    inbox_view::InboxView,
};

/// Every Account's Inbox as one, newest first, its newest `limit` rows or
/// all, ended by the latest cache edge of an Account that may hold older
/// mail, with that Account (ADR 0014); `None` before any Inbox is resynced.
pub fn read(
    caches: &[Cache],
    view: &InboxView,
    limit: Option<usize>,
) -> Result<Option<(Inbox, Option<usize>)>, cache::Error> {
    let (mut messages, mut elsewhere, mut edge) = (Vec::new(), Vec::new(), None);
    let mut synced = false;
    for (account, cache) in caches.iter().enumerate() {
        let Some(inbox) = cache.inbox(INBOX, limit)? else {
            continue;
        };

        synced = true;
        messages.extend(inbox.messages);
        elsewhere.extend(inbox.elsewhere);
        let bottomed = cache
            .oldest_uid(INBOX)?
            .is_none_or(|oldest| view.bottomed(account, INBOX, oldest));
        let arrival = cache.oldest_arrival(INBOX)?;
        if !bottomed && arrival > edge.map(|(at, _)| at) {
            edge = arrival.map(|at| (at, account));
        }
    }

    if !synced {
        return Ok(None);
    }

    if let Some((edge, _)) = edge {
        messages.retain(|m| m.received >= edge);
    }

    // Stable: of one arrival and UID, the first Account's goes first.
    messages.sort_by(|a, b| (b.received, b.uid).cmp(&(a.received, a.uid)));
    messages.truncate(limit.unwrap_or(usize::MAX));
    elsewhere.sort_by_key(|m| m.received);
    let inbox = Inbox {
        messages,
        elsewhere,
    };

    Ok(Some((inbox, edge.map(|(_, account)| account))))
}
