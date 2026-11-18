use futures::{StreamExt, channel::mpsc::UnboundedReceiver};

use crate::{backfill, cache::Cache, imap::MailServer, snippets, sync::Error, update::Update};

/// Work for the fetch connection.
pub enum Request {
    Bodies(BodyRequest),
    /// The page of Messages older than every cached one.
    Backfill,
}

/// Bodies the Preview needs. The selected one comes alone, so it does not
/// wait on its neighbours.
pub struct BodyRequest {
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

/// Serves `requests` until the channel closes, telling `send` how each
/// went: bodies first, then a page of older Messages if asked for, then one
/// batch of a page's Snippets. A refused batch of bodies leaves the
/// connection alone; any other failure ends the serving.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn serve(
    server: &mut impl MailServer,
    cache: &Cache,
    requests: &mut UnboundedReceiver<Request>,
    send: impl Fn(Update),
) -> Result<(), Error> {
    let mut waiting = Vec::new();
    let mut older = false;
    // Rows below this UID still wait for their Snippets.
    let mut snippets_below = None;
    loop {
        if waiting.is_empty() && !older && snippets_below.is_none() {
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
            let selected = Vec::from_iter(request.selected);
            for uids in [selected, request.neighbours] {
                if uids.is_empty() {
                    continue;
                }

                // A `NO` names no UID: ask again one by one, so a single
                // body the server refuses does not take its batch with it.
                match fetch(server, cache, uids).await? {
                    Fetched::Refused { uids, .. } if uids.len() > 1 => {
                        for uid in uids {
                            send(Update::Bodies(fetch(server, cache, vec![uid]).await?));
                        }
                    }
                    done => send(Update::Bodies(done)),
                }
            }
        }

        if std::mem::take(&mut older) {
            let report = |backfill| send(Update::Backfill(backfill));
            let below = backfill::page(server, cache, report, || send(Update::Inbox)).await?;
            // The higher bound keeps an earlier page's uncut rows in.
            snippets_below = snippets_below.max(below);
        } else if let Some(below) = snippets_below {
            // One batch at a time, so a body asked for meanwhile goes next.
            let written = || send(Update::Inbox);
            if !snippets::batch(server, cache, Some(below), written).await? {
                snippets_below = None;
            }
        }
    }
}

fn take(request: Request, waiting: &mut Vec<BodyRequest>, older: &mut bool) {
    match request {
        Request::Bodies(bodies) => waiting.push(bodies),
        Request::Backfill => *older = true,
    }
}

#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn fetch(
    server: &mut impl MailServer,
    cache: &Cache,
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

    cache.write_bodies(&bodies)?;
    tracing::debug!("fetched {} of {} bodies", bodies.len(), uids.len());
    Ok(Fetched::Stored(
        bodies.into_iter().map(|(uid, _)| uid).collect(),
    ))
}
