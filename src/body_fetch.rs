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
    /// The body of this UID in this Folder, which a Reply waits on: served
    /// first, and never passed over for a later request.
    Reply(Arc<str>, u32),
}

/// The requests received and not yet served.
#[derive(Default)]
struct Pending {
    bodies: Option<BodyRequest>,
    older: Option<Arc<str>>,
    reply: Option<(Arc<str>, u32)>,
}

impl Pending {
    const fn is_empty(&self) -> bool {
        self.bodies.is_none() && self.older.is_none() && self.reply.is_none()
    }

    fn take(&mut self, request: Request) {
        match request {
            Request::Bodies(bodies) => self.bodies = Some(bodies),
            Request::Backfill(folder) => self.older = Some(folder),
            Request::Reply(folder, uid) => self.reply = Some((folder, uid)),
        }
    }

    fn receive(&mut self, requests: &mut UnboundedReceiver<Request>) {
        while let Ok(more) = requests.try_recv() {
            self.take(more);
        }
    }
}

/// Bodies the Preview needs, by UID in `folder`. Each request replaces the
/// one before it, so it names every body still missing. The selected one
/// comes alone, so it does not wait on its neighbours.
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
/// a Reply's body, bodies, then a page of older Messages, then one batch of
/// its Snippets, each in its own Folder. Only a failure other than a refusal
/// ends the serving.
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
    let mut pending = Pending::default();
    // Rows of this Folder below this UID still wait for their Snippets.
    let mut snippets_below: Option<(Arc<str>, u32)> = None;
    loop {
        if pending.is_empty() && snippets_below.is_none() {
            let Some(first) = requests.next().await else {
                return Ok(());
            };

            pending.take(first);
        }

        pending.receive(requests);
        if let Some(reply) = pending.reply.take() {
            fetch_reply(server, cache, (&mut examined, reply), &send).await?;
        }

        while let Some(request) = pending.bodies.take() {
            if !open(server, &mut examined, &request.folder).await? {
                continue;
            }

            let BodyRequest {
                folder,
                selected,
                neighbours,
            } = request;
            if let Some(uid) = selected {
                fetch_each(server, cache, &folder, vec![uid], &send).await?;
            }

            // The neighbours of a selection already left behind are not fetched.
            pending.receive(requests);
            if pending.bodies.is_none() && !neighbours.is_empty() {
                fetch_each(server, cache, &folder, neighbours, &send).await?;
            }

            pending.receive(requests);
            if pending.reply.is_some() {
                break;
            }
        }

        if pending.reply.is_some() {
            continue;
        }

        if let Some(folder) = pending.older.take() {
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

/// Fetches the body a Reply waits on; one whose Folder is gone is refused.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn fetch_reply(
    server: &mut impl MailServer,
    cache: &Cache,
    (examined, (folder, uid)): (&mut Option<Arc<str>>, (Arc<str>, u32)),
    send: &impl Fn(Update),
) -> Result<(), Error> {
    if open(server, examined, &folder).await? {
        return fetch_each(server, cache, &folder, vec![uid], send).await;
    }

    let gone = Fetched::Refused {
        uids: vec![uid],
        reason: "its Folder is gone".to_owned(),
    };

    send(Update::Bodies(folder, gone));
    Ok(())
}

/// Fetches `uids` and says how it went. A `NO` names no UID: they are asked
/// again one by one, so a single refused body does not take its batch with it.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn fetch_each(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &Arc<str>,
    uids: Vec<u32>,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    match fetch(server, cache, folder, uids).await? {
        Fetched::Refused { uids, .. } if uids.len() > 1 => {
            for uid in uids {
                let fetched = fetch(server, cache, folder, vec![uid]).await?;
                send(Update::Bodies(Arc::clone(folder), fetched));
            }
        }
        done => send(Update::Bodies(Arc::clone(folder), done)),
    }

    Ok(())
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
