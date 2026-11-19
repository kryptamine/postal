use std::sync::Arc;

use crate::{
    cache::{Cache, PendingMove},
    folder::Role,
    imap::MailServer,
    sync::{self, Error},
    update::Update,
};

/// Makes every move the Cache queues for the server, selecting the Folder
/// each starts from. Takes the Folder selected now and returns it, or `None`
/// once another was selected: what that one announced is not the shown one's.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn send(
    server: &mut impl MailServer,
    cache: &Cache,
    mut selected: Option<Arc<str>>,
    send: &impl Fn(Update),
) -> Result<Option<Arc<str>>, Error> {
    let pending = cache.pending_moves()?;
    if pending.is_empty() {
        return Ok(selected);
    }

    let (mut trash, mut reselected) = (None, false);
    for group in pending.chunk_by(|a, b| (&a.from, &a.to) == (&b.from, &b.to)) {
        let first = &group[0];
        if first.to.is_none() && trash.is_none() {
            trash = find_trash(server, cache, send).await?;
        }

        let Some(to) = first.to.clone().or_else(|| trash.clone()) else {
            tracing::warn!(
                "no Folder has the Trash role: {} Deletes taken back",
                group.len()
            );
            group.iter().try_for_each(|m| cache.unmove(m.id))?;
            send(Update::NoTrash);
            continue;
        };

        if selected.as_deref() != Some(&*first.from) {
            // A failed SELECT leaves no Folder selected (RFC 3501 6.3.1).
            (selected, reselected) = (None, true);
            match sync::select(server, &first.from).await {
                Ok(_) => selected = Some(Arc::clone(&first.from)),
                Err(Error::Gone(error)) => {
                    tracing::warn!("{} is gone, its moves with it: {error}", first.from);
                    refused(cache, group, send)?;
                    continue;
                }
                Err(error) => return Err(error),
            }
        }

        let uids: Vec<u32> = group.iter().map(|m| m.uid).collect();
        let moved = match server.move_to(&uids, &to).await {
            Err(error) if error.refusal().is_some() => {
                tracing::warn!("moving out of {} was refused: {error}", first.from);
                refused(cache, group, send)?;
                continue;
            }
            result => result?,
        };

        if !record(cache, group, &to, &moved)? {
            send(Update::Unrestored);
        }
    }

    send(Update::Inbox);
    Ok(selected.filter(|_| !reselected))
}

/// Writes what the server made of `group`, moved to `to` under the UIDs
/// `moved` names; false when an Undo found its Message out of reach.
fn record(
    cache: &Cache,
    group: &[PendingMove],
    to: &str,
    moved: &[(u32, u32)],
) -> Result<bool, Error> {
    let there = |uid| {
        moved
            .iter()
            .find(|&&(from, _)| from == uid)
            .map(|&(_, to)| to)
    };
    let mut restored = true;
    for m in group {
        // A server with UIDPLUS names each Message it moved.
        if m.to.is_some() {
            restored &= there(m.uid).is_some();
            cache.moved_back(m.id, there(m.uid).is_some())?;
        } else {
            restored &= cache.moved(m.id, to, there(m.uid))?;
        }
    }

    Ok(restored)
}

/// The Folder with the Trash role, listing the Folders first when the
/// Cache knows none.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn find_trash(
    server: &mut impl MailServer,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<Option<Arc<str>>, Error> {
    let trash = |cache: &Cache| -> Result<Option<Arc<str>>, Error> {
        let folders = cache.folders()?;
        Ok(folders
            .into_iter()
            .find(|f| f.role == Some(Role::Trash))
            .map(|f| f.name))
    };

    if let Some(found) = trash(cache)? {
        return Ok(Some(found));
    }

    cache.list_folders(&server.folders().await?)?;
    send(Update::Folders);
    trash(cache)
}

/// Takes back the Deletes of `group`, or drops its Undos: the Messages
/// they would restore are out of reach.
fn refused(cache: &Cache, group: &[PendingMove], send: &impl Fn(Update)) -> Result<(), Error> {
    for m in group {
        if m.to.is_some() {
            cache.moved_back(m.id, false)?;
        } else {
            cache.unmove(m.id)?;
        }
    }

    if group.iter().any(|m| m.to.is_some()) {
        send(Update::Unrestored);
    }

    Ok(())
}
