use std::sync::Arc;

use crate::{
    cache::{Cache, Destination, PendingMove},
    folder::Role,
    imap::MailServer,
    imap::Moved,
    role_folder,
    sync::{self, Error},
    uid::Uid,
    update::Update,
};

/// Makes every move the Cache queues for the server, selecting the Folder
/// each starts from. Takes the Folder selected now and returns it, or `None`
/// once another was selected: what that one announced is not the shown one's.
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
        let to = match &first.to {
            Destination::Back(folder) => Some(Arc::clone(folder)),
            Destination::Trash => {
                if trash.is_none() {
                    trash = role_folder::find(server, cache, Role::Trash, send).await?;
                }

                trash.clone()
            }
        };

        let Some(to) = to else {
            tracing::warn!(
                "no Folder has the Trash role: {} Deletes taken back",
                group.len()
            );
            refused(cache, group, send)?;
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

        let uids: Vec<Uid> = group.iter().map(|m| m.uid).collect();
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

/// Writes where the server put `group`, into `to` under the UIDs `moved`
/// names; false when an Undo's Message is out of reach.
fn record(cache: &Cache, group: &[PendingMove], to: &str, moved: &[Moved]) -> Result<bool, Error> {
    // A server with UIDPLUS names each Message it moved.
    let there = |uid| {
        moved
            .iter()
            .find(|moved| moved.from == uid)
            .map(|moved| moved.to)
    };
    let mut reached = true;
    for m in group {
        reached &= cache.arrived(m, to, there(m.uid))?;
    }

    Ok(reached)
}

/// Leaves every Message of `group` where the server has it.
fn refused(cache: &Cache, group: &[PendingMove], send: &impl Fn(Update)) -> Result<(), Error> {
    let mut reached = true;
    for m in group {
        reached &= cache.refused(m)?;
    }

    if !reached {
        send(Update::Unrestored);
    }

    Ok(())
}
