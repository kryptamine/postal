use std::collections::HashMap;

use crate::{
    cache::Cache,
    domain::body_key::BodyKey,
    domain::folder::{Folder, Role},
    domain::folder_name::FolderName,
    domain::thread_id::ThreadId,
    domain::uid::Uid,
    mail::fetch::one_by_one,
    mail::imap::{ImapSession, Mode},
    mail::passes::snippets,
    mail::sync::{self, Error},
    mail::update::Update,
};

/// Caches the Messages of Gmail's Thread `id` as All Mail holds them, with
/// their bodies and Snippets, leaving All Mail open. A refusal is only
/// logged. See docs/adr/0009-threads-from-gmail-thread-id.md.
pub async fn fetch(
    server: &mut ImapSession,
    cache: &Cache,
    id: ThreadId,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    match fetched(server, cache, id, send).await {
        Err(Error::Imap(error)) if error.refusal().is_some() => {
            tracing::warn!("fetching Thread {id} was refused: {error}");
            Ok(())
        }
        Err(Error::Gone(error)) => {
            tracing::warn!("All Mail is gone: {error}");
            Ok(())
        }
        result => result,
    }
}

async fn fetched(
    server: &mut ImapSession,
    cache: &Cache,
    id: ThreadId,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let Some(all) = all_mail(server, cache, send).await? else {
        tracing::warn!("the server lists no All Mail: Thread {id} not fetched");
        return Ok(());
    };

    sync::open(server, &all, Mode::Examine).await?;
    let uids = server.thread(id).await?;
    let found = if uids.is_empty() {
        Vec::new()
    } else {
        server.by_uid(&uids).await?
    };

    let mut missing = Vec::new();
    for new in &found {
        if !cache.has_body(new.row.body_key)? {
            missing.push(new.row.uid);
        }
    }

    let keys: HashMap<Uid, BodyKey> = found.iter().map(|m| (m.row.uid, m.row.body_key)).collect();
    let fetched = one_by_one::fetch(&missing, "body", async |uids| server.bodies(uids).await);
    let bodies: Vec<_> = fetched
        .await?
        .into_iter()
        .filter_map(|(uid, raw)| Some((*keys.get(&uid)?, raw)))
        .collect();

    let (mut rows, mut pending, mut learnt) = (Vec::new(), Vec::new(), Vec::new());
    for new in found {
        if let Some(part) = new.text_part {
            pending.push((new.row.uid, part));
        }

        rows.push(new.row);
        learnt.push(new.correspondence);
    }

    let mut snippets: HashMap<_, _> = snippets::cut(server, &pending).await?.into_iter().collect();
    for row in &mut rows {
        row.snippet = snippets.remove(&row.uid).unwrap_or_default();
    }

    cache.write_thread(id, &rows, &learnt, &bodies)?;
    tracing::info!("Thread {id} holds {} Messages in All Mail", rows.len());
    send(Update::Inbox);
    Ok(())
}

/// The name of the Folder that holds every Message but those in Trash and
/// Spam, listing the Folders first when the Cache knows none.
async fn all_mail(
    server: &mut ImapSession,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<Option<FolderName>, Error> {
    if let Some(name) = all(cache.folders()?) {
        return Ok(Some(name));
    }

    let listed = server.folders().await?;
    cache.list_folders(&listed)?;
    send(Update::Folders);
    Ok(all(listed))
}

fn all<A>(folders: Vec<Folder<A>>) -> Option<FolderName> {
    folders
        .into_iter()
        .find(|f| f.role == Some(Role::All))
        .map(|f| f.name)
}
