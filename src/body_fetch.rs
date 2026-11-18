use futures::{StreamExt, channel::mpsc::UnboundedReceiver};

use crate::{cache::Cache, imap::MailServer, sync::Error};

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

/// Fetches the bodies `requests` name into the Cache until the channel
/// closes, telling `fetched` how each batch ended. A refused batch leaves
/// the connection alone; any other failure ends the serving.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn serve(
    server: &mut impl MailServer,
    cache: &Cache,
    requests: &mut UnboundedReceiver<BodyRequest>,
    fetched: impl Fn(Fetched),
) -> Result<(), Error> {
    let mut waiting = Vec::new();
    loop {
        if waiting.is_empty() {
            let Some(first) = requests.next().await else {
                return Ok(());
            };

            waiting.push(first);
        }

        while let Ok(more) = requests.try_recv() {
            waiting.push(more);
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
                            fetched(fetch(server, cache, vec![uid]).await?);
                        }
                    }
                    done => fetched(done),
                }
            }
        }
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
