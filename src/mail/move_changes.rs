use crate::{
    cache::{Cache, Destination, PendingMove},
    domain::folder::Role,
    domain::folder_name::FolderName,
    domain::uid::Uid,
    mail::imap::{MailServer, Mode, Moved},
    mail::role_folder,
    mail::sync::{self, Error},
    mail::update::Update,
};

/// Makes every move the Cache queues for the server, selecting the Folder
/// each starts from.
pub async fn send(
    server: &mut impl MailServer,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let pending = cache.pending_moves()?;
    if pending.is_empty() {
        return Ok(());
    }

    let mut trash = None;
    for group in pending.chunk_by(|a, b| (&a.from, &a.to) == (&b.from, &b.to)) {
        let first = &group[0];
        let to = match &first.to {
            Destination::Back(folder) => Some(folder.clone()),
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

        match sync::open(server, &first.from, Mode::Select).await {
            Err(Error::Gone(error)) => {
                tracing::warn!("{} is gone, its moves with it: {error}", first.from);
                refused(cache, group, send)?;
                continue;
            }
            result => result?,
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
    Ok(())
}

/// Writes where the server put `group`, into `to` under the UIDs `moved`
/// names; false when an Undo's Message is out of reach.
fn record(
    cache: &Cache,
    group: &[PendingMove],
    to: &FolderName,
    moved: &[Moved],
) -> Result<bool, Error> {
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
