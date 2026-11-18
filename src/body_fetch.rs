use std::sync::Arc;

use futures::{StreamExt, channel::mpsc::UnboundedReceiver};

use crate::{
    backfill,
    cache::Cache,
    imap::MailServer,
    snippets,
    sync::{self, Error},
    update::Update,
};

/// Work for the fetch connection.
pub enum Request {
    Bodies(BodyRequest),
    /// The page of Messages of this Folder older than every cached one.
    Backfill(Arc<str>),
}

/// Bodies the Preview needs, by UID in `folder`. The selected one comes
/// alone, so it does not wait on its neighbours.
pub struct BodyRequest {
    pub folder: Arc<str>,
    pub selected: Option<u32>,
    pub neighbours: Vec<u32>,
}

/// How one fetch of bodies ended.
pub enum Fetched {
    /// These UIDs' bodies are now in the Cache.
    Stored(Vec<u32>),
    /// The server answered `NO` for these UIDs, with this reason.
    Refused { uids: Vec<u32>, reason: String },
}

/// Serves `requests` until the channel closes, telling `send` how each went:
/// bodies, then a page of older Messages, then one batch of its Snippets, each
/// in its own Folder. Only a failure other than a refusal ends the serving.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn serve(
    server: &mut impl MailServer,
    cache: &Cache,
    (requests, mut examined): (&mut UnboundedReceiver<Request>, Option<Arc<str>>),
    send: impl Fn(Update),
) -> Result<(), Error> {
    let mut waiting = Vec::new();
    let mut older = None;
    // Rows of this Folder below this UID still wait for their Snippets.
    let mut snippets_below: Option<(Arc<str>, u32)> = None;
    loop {
        if waiting.is_empty() && older.is_none() && snippets_below.is_none() {
            let Some(first) = requests.next().await else {
                return Ok(());
            };

            take(first, &mut waiting, &mut older);
        }

        while let Ok(more) = requests.try_recv() {
            take(more, &mut waiting, &mut older);
        }

        // The newest request is the selection on screen; the rest can wait.
        while let Some(request) = waiting.pop() {
            if !open(server, &mut examined, &request.folder).await? {
                continue;
            }

            let folder = &request.folder;
            let selected = Vec::from_iter(request.selected);
            for uids in [selected, request.neighbours] {
                if uids.is_empty() {
                    continue;
                }

                // A `NO` names no UID: ask again one by one, so a single
                // body the server refuses does not take its batch with it.
                match fetch(server, cache, folder, uids).await? {
                    Fetched::Refused { uids, .. } if uids.len() > 1 => {
                        for uid in uids {
                            let fetched = fetch(server, cache, folder, vec![uid]).await?;
                            send(Update::Bodies(Arc::clone(folder), fetched));
                        }
                    }
                    done => send(Update::Bodies(Arc::clone(folder), done)),
                }
            }
        }

        if let Some(folder) = older.take() {
            let report = |backfill| send(Update::Backfill(backfill));
            let written = || send(Update::Inbox);
            // The page examines the Folder itself, and may find it gone.
            examined = None;
            let below = backfill::page(server, cache, &folder, report, written).await?;
            // The higher bound keeps an earlier page's uncut rows in.
            snippets_below = match (snippets_below, below) {
                (Some((same, earlier)), Some(below)) if same == folder => {
                    Some((same, earlier.max(below)))
                }
                (kept, None) => kept,
                (_, Some(below)) => Some((folder, below)),
            };
        } else if let Some((folder, below)) = snippets_below.clone() {
            // One batch at a time, so a body asked for meanwhile goes next.
            let written = || send(Update::Inbox);
            if !open(server, &mut examined, &folder).await?
                || !snippets::batch(server, cache, &folder, Some(below), written).await?
            {
                snippets_below = None;
            }
        }
    }
}

fn take(request: Request, waiting: &mut Vec<BodyRequest>, older: &mut Option<Arc<str>>) {
    match request {
        Request::Bodies(bodies) => waiting.push(bodies),
        Request::Backfill(folder) => *older = Some(folder),
    }
}

/// Examines `folder` unless it is `examined` already; false when the
/// server no longer has it.
async fn open(
    server: &mut impl MailServer,
    examined: &mut Option<Arc<str>>,
    folder: &Arc<str>,
) -> Result<bool, Error> {
    if examined.as_ref() == Some(folder) {
        return Ok(true);
    }

    *examined = None;
    match sync::examine(server, folder).await {
        Ok(_) => {
            *examined = Some(Arc::clone(folder));
            Ok(true)
        }
        Err(Error::Gone(error)) => {
            tracing::warn!("{folder} is gone: {error}");
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn fetch(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &str,
    uids: Vec<u32>,
) -> Result<Fetched, Error> {
    let bodies = match server.bodies(&uids).await {
        Ok(bodies) => bodies,
        Err(error) => {
            let Some(reason) = error.refusal() else {
                return Err(error.into());
            };

            tracing::warn!("fetching the bodies of {uids:?} was refused: {error}");
            let reason = reason.to_owned();
            return Ok(Fetched::Refused { uids, reason });
        }
    };

    cache.write_bodies(folder, &bodies)?;
    tracing::debug!("fetched {} of {} bodies", bodies.len(), uids.len());
    Ok(Fetched::Stored(
        bodies.into_iter().map(|(uid, _)| uid).collect(),
    ))
}
