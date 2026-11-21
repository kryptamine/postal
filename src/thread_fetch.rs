use std::{collections::HashMap, sync::Arc};

use crate::{
    cache::Cache,
    folder::{Folder, Role},
    imap::MailServer,
    snippets,
    sync::{self, Error},
    update::Update,
};

/// Caches the Messages of Gmail's Thread `id` as All Mail holds them, with
/// their bodies and Snippets, leaving All Mail examined. A refusal is only
/// logged. See docs/adr/0009-threads-from-gmail-thread-id.md.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn fetch(
    server: &mut impl MailServer,
    cache: &Cache,
    id: u64,
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

#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn fetched(
    server: &mut impl MailServer,
    cache: &Cache,
    id: u64,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let Some(all) = all_mail(server, cache, send).await? else {
        tracing::warn!("the server lists no All Mail: Thread {id} not fetched");
        return Ok(());
    };

    sync::examine(server, &all).await?;
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

    let keys: HashMap<u32, i64> = found.iter().map(|m| (m.row.uid, m.row.body_key)).collect();
    let bodies: Vec<_> = bodies(server, &missing)
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

    cache.write_thread(id, (&rows, &learnt), &bodies)?;
    tracing::info!("Thread {id} holds {} Messages in All Mail", rows.len());
    send(Update::Inbox);
    Ok(())
}

/// The name of the Folder that holds every Message but those in Trash and
/// Spam, listing the Folders first when the Cache knows none.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn all_mail(
    server: &mut impl MailServer,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<Option<Arc<str>>, Error> {
    let find = |folders: Vec<Folder>| {
        folders
            .into_iter()
            .find(|f| f.role == Some(Role::All))
            .map(|f| f.name)
    };

    if let Some(name) = find(cache.folders()?) {
        return Ok(Some(name));
    }

    let listed = server.folders().await?;
    cache.list_folders(&listed)?;
    send(Update::Folders);
    Ok(find(listed))
}

/// The bodies of `uids`; when the server refuses the batch, each is asked
/// again alone, so one refused body does not take the rest with it.
async fn bodies(server: &mut impl MailServer, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, Error> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }

    match server.bodies(uids).await {
        Err(error) if error.refusal().is_some() && uids.len() > 1 => {
            let mut bodies = Vec::new();
            for &uid in uids {
                match server.bodies(&[uid]).await {
                    Ok(one) => bodies.extend(one),
                    Err(error) if error.refusal().is_some() => {
                        tracing::warn!("fetching the body of {uid} was refused: {error}");
                    }
                    Err(error) => return Err(error.into()),
                }
            }

            Ok(bodies)
        }
        result => Ok(result?),
    }
}
