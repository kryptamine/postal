use std::{sync::Arc, time::Duration};

use futures::{StreamExt, channel::mpsc::UnboundedReceiver};

use crate::{
    backfill,
    cache::Cache,
    imap::MailServer,
    spare_work::SpareWork,
    sync::{self, Error},
    update::Update,
};

/// Work for the fetch connection.
pub enum Request {
    Bodies(BodyRequest),
    /// The page of Messages of this Folder older than every cached one.
    Backfill(Arc<str>),
    /// The bodies of these UIDs, each in its Folder, which a Reply waits
    /// on: served first, and never passed over for a later request.
    Reply(Vec<(Arc<str>, u32)>),
}

/// The requests received and not yet served.
#[derive(Default)]
struct Pending {
    bodies: Option<BodyRequest>,
    older: Option<Arc<str>>,
    reply: Option<Vec<(Arc<str>, u32)>>,
}

impl Pending {
    const fn is_empty(&self) -> bool {
        self.bodies.is_none() && self.older.is_none() && self.reply.is_none()
    }

    fn take(&mut self, request: Request) {
        match request {
            Request::Bodies(bodies) => self.bodies = Some(bodies),
            Request::Backfill(folder) => self.older = Some(folder),
            Request::Reply(bodies) => self.reply = Some(bodies),
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

impl Fetched {
    fn uids(&self) -> &[u32] {
        match self {
            Self::Stored(uids) | Self::Refused { uids, .. } => uids,
        }
    }
}

/// Serves `requests` until the channel closes, telling `send` how each went:
/// a Reply's body, bodies, then a page of older Messages, each in its own
/// Folder, and `SpareWork` while none waits. Only a failure other than a
/// refusal ends the serving.
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
    let (mut pending, mut spare) = (Pending::default(), SpareWork::new());
    loop {
        if pending.is_empty() {
            match next(requests, spare.patience()).await {
                Next::Request(request) => pending.take(request),
                Next::Spare => {
                    spare.step(server, cache, (&mut examined, &send)).await?;
                    continue;
                }
                Next::Closed => return Ok(()),
            }
        }

        pending.receive(requests);
        if let Some(reply) = pending.reply.take() {
            fetch_reply(server, cache, &mut examined, reply, &send).await?;
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
            spare.backfilled(folder, below);
        }
    }
}

enum Next {
    Request(Request),
    /// The patience asked for ran out first.
    Spare,
    Closed,
}

/// The next request, waiting no longer than `patience` when given.
async fn next(requests: &mut UnboundedReceiver<Request>, patience: Option<Duration>) -> Next {
    let next = match patience {
        Some(patience) => tokio::time::timeout(patience, requests.next()).await.ok(),
        None => Some(requests.next().await),
    };

    match next {
        Some(Some(request)) => Next::Request(request),
        Some(None) => Next::Closed,
        None => Next::Spare,
    }
}

/// Fetches the bodies a Reply waits on, a Folder at a time; those the
/// server no longer has, or whose Folder is gone, are refused.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn fetch_reply(
    server: &mut impl MailServer,
    cache: &Cache,
    examined: &mut Option<Arc<str>>,
    bodies: Vec<(Arc<str>, u32)>,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let mut folders: Vec<(Arc<str>, Vec<u32>)> = Vec::new();
    for (folder, uid) in bodies {
        match folders.iter_mut().find(|(f, _)| *f == folder) {
            Some((_, uids)) => uids.push(uid),
            None => folders.push((folder, vec![uid])),
        }
    }

    for (folder, uids) in folders {
        let (uids, reason) = if open(server, examined, &folder).await? {
            let answered = fetch_each(server, cache, &folder, uids.clone(), send).await?;
            let unanswered = uids.into_iter().filter(|u| !answered.contains(u));
            (
                unanswered.collect::<Vec<_>>(),
                "the server no longer has it",
            )
        } else {
            (uids, "its Folder is gone")
        };

        if uids.is_empty() {
            continue;
        }

        let gone = Fetched::Refused {
            uids,
            reason: reason.to_owned(),
        };

        send(Update::Bodies(folder, gone));
    }

    Ok(())
}

/// Fetches `uids` and says how it went. A `NO` names no UID: they are asked
/// again one by one, so a single refused body does not take its batch with it.
/// Returns the UIDs the server answered for, stored or refused.
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
) -> Result<Vec<u32>, Error> {
    let mut answered = Vec::new();
    match fetch(server, cache, folder, uids).await? {
        Fetched::Refused { uids, .. } if uids.len() > 1 => {
            for uid in uids {
                let fetched = fetch(server, cache, folder, vec![uid]).await?;
                answered.extend_from_slice(fetched.uids());
                send(Update::Bodies(Arc::clone(folder), fetched));
            }
        }
        done => {
            answered.extend_from_slice(done.uids());
            send(Update::Bodies(Arc::clone(folder), done));
        }
    }

    Ok(answered)
}

/// Examines `folder` unless it is `examined` already; false when the
/// server no longer has it.
pub async fn open(
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
