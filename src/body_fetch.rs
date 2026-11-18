use futures::{StreamExt, channel::mpsc::UnboundedReceiver};

use crate::{cache::Cache, imap::MailServer, sync::Error};

/// Bodies the Preview needs. The selected one comes alone, so it does not
/// wait on its neighbours.
pub struct BodyRequest {
    pub selected: Option<u32>,
    pub neighbours: Vec<u32>,
}

/// Fetches the bodies `requests` name into the Cache until the channel
/// closes, calling `stored` with the UIDs of each batch written.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn serve(
    server: &mut impl MailServer,
    cache: &Cache,
    mut requests: UnboundedReceiver<BodyRequest>,
    stored: impl Fn(Vec<u32>),
) -> Result<(), Error> {
    while let Some(first) = requests.next().await {
        let mut waiting = vec![first];
        while let Ok(more) = requests.try_recv() {
            waiting.push(more);
        }

        // The newest request is the selection on screen; the rest can wait.
        for request in waiting.into_iter().rev() {
            for uids in [Vec::from_iter(request.selected), request.neighbours] {
                if uids.is_empty() {
                    continue;
                }

                let bodies = server.bodies(&uids).await?;
                cache.write_bodies(&bodies)?;
                tracing::debug!("fetched {} of {} bodies", bodies.len(), uids.len());
                stored(bodies.into_iter().map(|(uid, _)| uid).collect());
            }
        }
    }

    Ok(())
}
