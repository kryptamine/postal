use crate::{
    cache::Cache,
    domain::folder_name::FolderName,
    domain::uid::Uid,
    mail::imap::{MailServer, Mode},
    mail::sync::{self, Error},
};

/// Messages per page of older ones the Message List asks for.
pub const PAGE: u32 = 500;

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

/// How a page of older Messages ended.
pub struct Page {
    /// The UID below which phase 2 has Snippets to cut, when it wrote rows.
    pub below: Option<Uid>,
    /// As `Backfill::Paged` has it.
    pub nothing_below: Option<Uid>,
}

/// Fetches the `size` Messages of `folder` just older than every cached one
/// (phase 1); `progress` follows it, `done` of `total`.
pub async fn page(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &FolderName,
    size: u32,
    progress: impl Fn(u32, u32),
) -> Result<Page, Error> {
    let Some(oldest) = cache.oldest_uid(folder)? else {
        return Ok(Page {
            below: None,
            nothing_below: None,
        });
    };

    let unreachable = Page {
        below: None,
        nothing_below: Some(oldest),
    };
    // A fresh EXAMINE, so the sequence numbers below are this moment's.
    let examined = match sync::reopen(server, folder, Mode::Examine).await {
        Err(Error::Gone(error)) => {
            tracing::warn!("{folder} is gone: {error}");
            return Ok(unreachable);
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
        _ => return Ok(unreachable),
    };

    let total = (before - 1).min(size);
    progress(0, total);
    let mut older = server
        .older(before, size, |done| progress(done, total))
        .await?;
    older.retain(|new| new.row.uid < oldest);
    // The sync connection may have replaced the rows while this one fetched.
    if !same(cache)? {
        return Ok(unreachable);
    }

    cache.backfill(folder, &older)?;
    tracing::info!("backfilled {} older Messages", older.len());
    let first = before.saturating_sub(size) <= 1;
    let new_oldest = older.iter().map(|new| new.row.uid).min().unwrap_or(oldest);
    Ok(Page {
        below: Some(oldest),
        nothing_below: first.then_some(new_oldest),
    })
}
