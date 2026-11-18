use std::cell::Cell;

use tokio::time::Instant;

use crate::{backoff::Backoff, status::Status, sync::Error, wake};

/// Runs `attempt` on a fresh connection after every outage, waiting out the
/// backoff first, until it fails for good. `attempt` reports `Live` once
/// through, which restarts the backoff; a wake from sleep retries at once.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn retry(
    mut attempt: impl AsyncFnMut(&dyn Fn(Status)) -> Result<(), Error>,
    report: impl Fn(Status),
) -> Result<(), Error> {
    let mut backoff = Backoff::default();
    loop {
        report(Status::Connecting);
        let live = Cell::new(false);
        let relay = |status| {
            live.set(live.get() || status == Status::Live);
            report(status);
        };

        let error = tokio::select! {
            result = attempt(&relay) => match result {
                Ok(()) => return Ok(()),
                Err(error) => error,
            },
            () = wake::woke() => {
                tracing::info!("woke from sleep: reconnecting");
                backoff = Backoff::default();
                continue;
            }
        };

        if live.get() {
            backoff = Backoff::default();
        }

        let Some(outage) = error.outage() else {
            return Err(error);
        };

        let wait = backoff.next();
        tracing::warn!("{error}: retrying in {} ms", wait.as_millis());
        let retry = Instant::now() + wait;
        report(Status::Retrying { outage, retry });
        tokio::select! {
            () = tokio::time::sleep_until(retry) => {}
            () = wake::woke() => {
                tracing::info!("woke from sleep: reconnecting");
                backoff = Backoff::default();
            }
        }
    }
}
