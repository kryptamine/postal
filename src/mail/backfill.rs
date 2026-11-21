use crate::{
    cache::Cache,
    domain::folder_name::FolderName,
    domain::uid::Uid,
    mail::imap::MailServer,
    mail::sync::{self, Error},
};

/// Messages per page of older ones.
const PAGE: u32 = 500;

/// How a page of older Messages is coming along.
pub enum Backfill {
    /// `done` of `total` rows so far.
    Fetching { done: u32, total: u32 },
    /// The rows are in the Cache of `folder`. `nothing_below` is its oldest
    /// cached UID when the server has nothing older, or nothing a page can
    /// reach now.
    Paged {
        folder: FolderName,
        nothing_below: Option<Uid>,
    },
}

/// Fetches the page of Messages of `folder` just older than every cached one (phase 1).
/// Returns the UID below which phase 2 has Snippets to cut, when it wrote
/// any rows. `report` follows phase 1; `written` follows the Cache write.
pub async fn page(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &FolderName,
    report: impl Fn(Backfill),
    written: impl Fn(),
) -> Result<Option<Uid>, Error> {
    let paged = |nothing_below| Backfill::Paged {
        folder: folder.clone(),
        nothing_below,
    };

    let Some(oldest) = cache.oldest_uid(folder)? else {
        report(paged(None));
        return Ok(None);
    };

    // A fresh EXAMINE, so the sequence numbers below are this moment's.
    let examined = match sync::examine(server, folder).await {
        Err(Error::Gone(error)) => {
            tracing::warn!("{folder} is gone: {error}");
            report(paged(Some(oldest)));
            return Ok(None);
        }
        examined => examined?,
    };

    // The sync connection refetches the Folder under a new UIDVALIDITY; an
    // oldest row the server lost is the IDLE connection's to remove.
    let same = |cache: &Cache| -> Result<bool, Error> {
        Ok(cache
            .state(folder)?
            .is_some_and(|known| known.uid_validity == examined.state.uid_validity))
    };

    let before = match server.sequence_number(oldest).await? {
        Some(before) if before > 1 && same(cache)? => before,
        _ => {
            report(paged(Some(oldest)));
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
        report(paged(Some(oldest)));
        return Ok(None);
    }

    cache.backfill(folder, &older)?;
    tracing::info!("backfilled {} older Messages", older.len());
    written();
    let first = before.saturating_sub(PAGE) <= 1;
    let new_oldest = older.iter().map(|new| new.row.uid).min().unwrap_or(oldest);
    report(paged(first.then_some(new_oldest)));
    Ok(Some(oldest))
}
