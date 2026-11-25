use std::cell::Cell;

use tokio::time::Instant;

use crate::{
    mail::backoff::Backoff, mail::outage::Outage, mail::refresh, mail::status::Status,
    mail::sync::Error, mail::wake,
};

/// Runs `attempt` on a fresh connection after every failure, after the
/// backoff, until the Account is refused. `Live` from `attempt` restarts the
/// backoff; a wake from sleep or a Refresh retries at once.
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

        // Only a new login helps here; a fresh connection starts any other
        // failure over, as a refusal by sequence numbers gone stale.
        if error.signed_out() {
            report(Status::SignedOut);
            return Err(error);
        }

        let outage = error.outage().unwrap_or(Outage::Failed);

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
            () = refresh::asked() => {
                tracing::info!("a refresh was asked: reconnecting");
                backoff = Backoff::default();
            }
        }
    }
}
