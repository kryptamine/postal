use futures::{StreamExt, channel::mpsc::UnboundedReceiver};

use crate::{cache::Cache, imap::MailServer, sync::Error};

/// Bodies the Preview needs. The selected one comes alone, so it does not
/// wait on its neighbours.
pub struct BodyRequest {
    pub selected: Option<u32>,
    pub neighbours: Vec<u32>,
}

/// Fetches the bodies `requests` name into the Cache until the channel
/// closes, calling `stored` with the UIDs of each batch written. `waiting`
/// goes first, and keeps what a failed fetch left unserved, oldest first.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn serve(
    server: &mut impl MailServer,
    cache: &Cache,
    requests: &mut UnboundedReceiver<BodyRequest>,
    waiting: &mut Vec<BodyRequest>,
    stored: impl Fn(Vec<u32>),
) -> Result<(), Error> {
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
        while let Some(request) = waiting.last() {
            let selected = Vec::from_iter(request.selected);
            for uids in [&selected, &request.neighbours] {
                if uids.is_empty() {
                    continue;
                }

                let bodies = server.bodies(uids).await?;
                cache.write_bodies(&bodies)?;
                tracing::debug!("fetched {} of {} bodies", bodies.len(), uids.len());
                stored(bodies.into_iter().map(|(uid, _)| uid).collect());
            }

            waiting.pop();
        }
    }
}
