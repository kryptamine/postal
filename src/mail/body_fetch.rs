use std::time::Duration;

use futures::{StreamExt, channel::mpsc::UnboundedReceiver};

use crate::{
    cache::Cache,
    domain::folder_name::FolderName,
    domain::message_ref::MessageRef,
    domain::uid::Uid,
    mail::backfill,
    mail::imap::{MailServer, Mode},
    mail::spare_work::SpareWork,
    mail::sync::{self, Error},
    mail::update::Update,
};

/// Work for the fetch connection.
pub enum Request {
    Bodies(BodyRequest),
    /// The page of Messages of this Folder older than every cached one.
    Backfill(FolderName),
    /// The bodies of these UIDs, each in its Folder, which a Reply waits
    /// on: served first, and never passed over for a later request.
    Reply(Vec<MessageRef>),
}

/// The requests received and not yet served.
#[derive(Default)]
struct Pending {
    bodies: Option<BodyRequest>,
    older: Option<FolderName>,
    reply: Option<Vec<MessageRef>>,
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
    pub folder: FolderName,
    pub selected: Option<Uid>,
    pub neighbours: Vec<Uid>,
}

/// How one fetch of bodies ended.
pub enum Fetched {
    /// These UIDs' bodies are now in the Cache.
    Stored(Vec<Uid>),
    /// The server answered `NO` for these UIDs, with this reason.
    Refused { uids: Vec<Uid>, reason: String },
}

impl Fetched {
    fn uids(&self) -> &[Uid] {
        match self {
            Self::Stored(uids) | Self::Refused { uids, .. } => uids,
        }
    }
}

/// Serves `requests` until the channel closes, telling `send` how each went:
/// a Reply's body, bodies, then a page of older Messages, each in its own
/// Folder, and `SpareWork` while none waits. Only a failure other than a
/// refusal ends the serving.
pub async fn serve(
    server: &mut impl MailServer,
    cache: &Cache,
    requests: &mut UnboundedReceiver<Request>,
    send: impl Fn(Update),
) -> Result<(), Error> {
    let (mut pending, mut spare) = (Pending::default(), SpareWork::new());
    loop {
        if pending.is_empty() {
            match next(requests, spare.patience()).await {
                Next::Request(request) => pending.take(request),
                Next::Spare => {
                    spare.step(server, cache, &send).await?;
                    continue;
                }
                Next::Closed => return Ok(()),
            }
        }

        pending.receive(requests);
        if let Some(reply) = pending.reply.take() {
            fetch_reply(server, cache, reply, &send).await?;
        }

        while let Some(request) = pending.bodies.take() {
            if !open(server, &request.folder).await? {
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
async fn fetch_reply(
    server: &mut impl MailServer,
    cache: &Cache,
    bodies: Vec<MessageRef>,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let mut folders: Vec<(FolderName, Vec<Uid>)> = Vec::new();
    for MessageRef { folder, uid } in bodies {
        match folders.iter_mut().find(|(f, _)| *f == folder) {
            Some((_, uids)) => uids.push(uid),
            None => folders.push((folder, vec![uid])),
        }
    }

    for (folder, uids) in folders {
        let (uids, reason) = if open(server, &folder).await? {
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
async fn fetch_each(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &FolderName,
    uids: Vec<Uid>,
    send: &impl Fn(Update),
) -> Result<Vec<Uid>, Error> {
    let mut answered = Vec::new();
    match fetch(server, cache, folder, uids).await? {
        Fetched::Refused { uids, .. } if uids.len() > 1 => {
            for uid in uids {
                let fetched = fetch(server, cache, folder, vec![uid]).await?;
                answered.extend_from_slice(fetched.uids());
                send(Update::Bodies(folder.clone(), fetched));
            }
        }
        done => {
            answered.extend_from_slice(done.uids());
            send(Update::Bodies(folder.clone(), done));
        }
    }

    Ok(answered)
}

/// Examines `folder` unless it is open already; false when the server no
/// longer has it.
pub async fn open(server: &mut impl MailServer, folder: &FolderName) -> Result<bool, Error> {
    match sync::open(server, folder, Mode::Examine).await {
        Ok(()) => Ok(true),
        Err(Error::Gone(error)) => {
            tracing::warn!("{folder} is gone: {error}");
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

async fn fetch(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &FolderName,
    uids: Vec<Uid>,
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
