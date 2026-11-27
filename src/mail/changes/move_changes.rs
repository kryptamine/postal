use crate::{
    cache::{Cache, Destination, Goal, PendingMove},
    domain::folder::Role,
    domain::folder_name::FolderName,
    domain::uid::Uid,
    mail::imap::{ImapSession, Mode, Moved},
    mail::role_folder,
    mail::sync::{self, Error},
    mail::update::Update,
};

/// Makes every move the Cache queues for the server, selecting the Folder
/// each starts from.
pub async fn send(
    server: &mut ImapSession,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let pending = cache.pending_moves()?;
    if pending.is_empty() {
        return Ok(());
    }

    let copies = Copies::of(server, cache)?;
    let mut found: Vec<(Role, Option<FolderName>)> = Vec::new();
    let mut landed: Vec<FolderName> = Vec::new();
    for group in pending.chunk_by(|a, b| (&a.from, &a.to) == (&b.from, &b.to)) {
        let first = &group[0];
        let to = destination(server, cache, &first.to, &mut found, send).await?;
        let Some(to) = to else {
            if let Destination::Goal(Goal::Role(role)) = first.to {
                tracing::warn!("no {role:?} Folder: {} moves taken back", group.len());
                send(Update::NoFolder(role));
            }

            refused(cache, group, send)?;
            continue;
        };

        // The role's Folder may turn out to be the one the Messages are in.
        if to == first.from {
            refused(cache, group, send)?;
            continue;
        }

        match sync::open(server, &first.from, Mode::Select).await {
            Err(Error::Gone(error)) => {
                tracing::warn!("{} is gone, its moves with it: {error}", first.from);
                refused(cache, group, send)?;
                continue;
            }
            result => result?,
        }

        let uids: Vec<Uid> = group.iter().map(|m| m.uid).collect();
        let moved = if copies.instead(&first.from, &to) {
            server.copy_to(&uids, &to).await
        } else {
            server.move_to(&uids, &to).await
        };

        let moved = match moved {
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

        if !landed.contains(&to) {
            landed.push(to);
        }
    }

    for folder in &landed {
        resync(server, cache, folder).await?;
    }

    send(Update::Inbox);
    Ok(())
}

/// Brings a cached Folder moved into up to the server, so the Messages
/// show there before it is opened; one never synced waits for its opening.
async fn resync(server: &mut ImapSession, cache: &Cache, folder: &FolderName) -> Result<(), Error> {
    if cache.state(folder)?.is_none() {
        return Ok(());
    }

    match sync::resync(server, cache, folder, Mode::Examine, &|_| {}).await {
        Err(Error::Gone(error)) => {
            tracing::warn!("{folder} is gone since the move into it: {error}");
            Ok(())
        }
        result => result.map(drop),
    }
}

/// On Gmail a move out of All Mail but to Trash or Spam is a copy:
/// docs/adr/0026-move-and-archive.md.
struct Copies {
    all_mail: Option<FolderName>,
    apart: Vec<FolderName>,
}

impl Copies {
    fn of(server: &ImapSession, cache: &Cache) -> Result<Self, Error> {
        let folders = if server.gmail() {
            cache.folders()?
        } else {
            Vec::new()
        };

        let named = |role| folders.iter().find(|f| f.role == Some(role));
        Ok(Self {
            all_mail: named(Role::All).map(|f| f.name.clone()),
            apart: [Role::Trash, Role::Spam]
                .into_iter()
                .filter_map(|role| named(role).map(|f| f.name.clone()))
                .collect(),
        })
    }

    fn instead(&self, from: &FolderName, to: &FolderName) -> bool {
        self.all_mail.as_ref() == Some(from) && !self.apart.contains(to)
    }
}

/// The Folder `to` names, each role's looked up once into `found`.
async fn destination(
    server: &mut ImapSession,
    cache: &Cache,
    to: &Destination,
    found: &mut Vec<(Role, Option<FolderName>)>,
    send: &impl Fn(Update),
) -> Result<Option<FolderName>, Error> {
    let role = match to {
        Destination::Back(folder) | Destination::Goal(Goal::Folder(folder)) => {
            return Ok(Some(folder.clone()));
        }
        Destination::Goal(Goal::Role(role)) => *role,
    };

    if let Some((_, folder)) = found.iter().find(|(r, _)| *r == role) {
        return Ok(folder.clone());
    }

    let folder = with_role(server, cache, role, send).await?;
    found.push((role, folder.clone()));
    Ok(folder)
}

/// The Folder with `role`; for Archive on Gmail, All Mail, as archiving
/// there takes the Inbox label off.
async fn with_role(
    server: &mut ImapSession,
    cache: &Cache,
    role: Role,
    send: &impl Fn(Update),
) -> Result<Option<FolderName>, Error> {
    let found = role_folder::find(server, cache, role, send).await?;
    if found.is_none() && role == Role::Archive && server.gmail() {
        return role_folder::find(server, cache, Role::All, send).await;
    }

    Ok(found)
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
