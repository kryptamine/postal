use crate::{cache::Cache, imap::MailServer, sync::Error};

/// Messages per page of older ones.
const PAGE: u32 = 500;

/// How a page of older Messages is coming along.
#[derive(Clone, Copy)]
pub enum Backfill {
    /// `done` of `total` rows so far.
    Fetching { done: u32, total: u32 },
    /// The rows are in the Cache. `nothing_below` is the oldest cached UID
    /// when the server has nothing older, or nothing a page can reach now.
    Paged { nothing_below: Option<u32> },
}

/// Fetches the page of Messages just older than every cached one (phase 1).
/// Returns the UID below which phase 2 has Snippets to cut, when it wrote
/// any rows. `report` follows phase 1; `written` follows the Cache write.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn page(
    server: &mut impl MailServer,
    cache: &Cache,
    report: impl Fn(Backfill),
    written: impl Fn(),
) -> Result<Option<u32>, Error> {
    let Some(oldest) = cache.oldest_uid()? else {
        report(Backfill::Paged {
            nothing_below: None,
        });
        return Ok(None);
    };

    // A fresh EXAMINE, so the sequence numbers below are this moment's.
    let examined = server.examine().await?;
    let nothing = Backfill::Paged {
        nothing_below: Some(oldest),
    };

    // The sync connection refetches the Inbox under a new UIDVALIDITY; an
    // oldest row the server lost is the IDLE connection's to remove.
    let same = |cache: &Cache| -> Result<bool, Error> {
        Ok(cache
            .state()?
            .is_some_and(|known| known.uid_validity == examined.state.uid_validity))
    };

    let before = match server.sequence_number(oldest).await? {
        Some(before) if before > 1 && same(cache)? => before,
        _ => {
            report(nothing);
            return Ok(None);
        }
    };

    let total = (before - 1).min(PAGE);
    report(Backfill::Fetching { done: 0, total });
    let progress = |done| report(Backfill::Fetching { done, total });
    let mut older = server.older(before, PAGE, progress).await?;
    older.retain(|new| new.row.uid < oldest);
    // The sync connection may have replaced the rows while this one fetched.
    if !same(cache)? {
        report(nothing);
        return Ok(None);
    }

    cache.backfill(&older)?;
    tracing::info!("backfilled {} older Messages", older.len());
    written();
    let first = before.saturating_sub(PAGE) <= 1;
    let new_oldest = older.iter().map(|new| new.row.uid).min().unwrap_or(oldest);
    report(Backfill::Paged {
        nothing_below: first.then_some(new_oldest),
    });
    Ok(Some(oldest))
}
