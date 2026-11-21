use crate::{
    cache::{self, Cache, Changes},
    imap::{self, Examined, MailServer},
    outage::Outage,
    status::Status,
    sync_state::SyncState,
};

/// The first run, and a run after a UIDVALIDITY change, fetches this many of
/// the newest Messages.
const WINDOW: u32 = 500;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Imap(#[from] imap::Error),
    #[error(transparent)]
    Cache(#[from] cache::Error),
    /// `EXAMINE` or `SELECT` was refused: the server no longer has the Folder.
    #[error(transparent)]
    Gone(imap::Error),
}

impl Error {
    /// `None` when retrying would fail the same way.
    pub fn outage(&self) -> Option<Outage> {
        match self {
            Self::Imap(source) => source.outage(),
            Self::Cache(_) | Self::Gone(_) => None,
        }
    }

    pub fn signed_out(&self) -> bool {
        matches!(self, Self::Imap(source) if source.signed_out())
    }
}

/// Examines `folder` and returns what it saw; `Gone` when refused.
pub async fn examine(server: &mut impl MailServer, folder: &str) -> Result<Examined, Error> {
    server.examine(folder).await.map_err(gone)
}

/// As `examine`, opening `folder` for flag changes too.
pub async fn select(server: &mut impl MailServer, folder: &str) -> Result<Examined, Error> {
    server.select(folder).await.map_err(gone)
}

/// A refusal to open a Folder means the server no longer has it.
fn gone(error: imap::Error) -> Error {
    if error.refusal().is_some() {
        Error::Gone(error)
    } else {
        Error::Imap(error)
    }
}

/// Brings the Cache up to the server's `folder` and writes the result in one
/// transaction, so the Message List never shows a half-applied resync. Returns
/// what its `SELECT` saw; `report` follows the first fetch, then says `Live`.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn resync(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &str,
    report: &dyn Fn(Status),
) -> Result<Examined, Error> {
    // Selected, not examined, so a Mark Read can be stored: ADR 0011.
    let examined = select(server, folder).await?;
    let unread = server.unread_count().await?;
    let known = cache.state(folder)?;
    let cached = cache.uids(folder)?;
    let changes = match known {
        Some(known) if known.uid_validity == examined.state.uid_validity && !cached.is_empty() => {
            update(server, &examined, known, &cached, unread).await?
        }
        _ => {
            if let Some(known) = known
                && known.uid_validity != examined.state.uid_validity
            {
                tracing::info!(
                    "UIDVALIDITY of {folder} changed from {} to {}: refetching it",
                    known.uid_validity,
                    examined.state.uid_validity
                );
            }

            refetch(server, (folder, &examined), unread, report).await?
        }
    };

    cache.apply(folder, &changes)?;
    report(Status::Live);
    Ok(examined)
}

#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn refetch(
    server: &mut impl MailServer,
    (folder, examined): (&str, &Examined),
    unread: usize,
    report: &dyn Fn(Status),
) -> Result<Changes, Error> {
    let new = if examined.exists == 0 {
        tracing::info!("{folder} is empty");
        Vec::new()
    } else {
        let total = examined.exists.min(WINDOW);
        report(Status::Syncing { done: 0, total });
        let progress = |done| report(Status::Syncing { done, total });
        server.newest(examined.exists, WINDOW, progress).await?
    };

    Ok(Changes {
        state: examined.state,
        exists: examined.exists,
        unread,
        replace: true,
        new,
        expunged: Vec::new(),
        flags: Vec::new(),
    })
}

/// New mail by UID range, expunges over the cached range, then flags.
/// `cached` is ascending and not empty.
async fn update(
    server: &mut impl MailServer,
    examined: &Examined,
    known: SyncState,
    cached: &[u32],
    unread: usize,
) -> Result<Changes, Error> {
    let (first, last) = (cached[0], cached[cached.len() - 1]);
    let new = if examined.state.uid_next.is_some() && examined.state.uid_next == known.uid_next {
        Vec::new()
    } else {
        server.since_uid(last + 1).await?
    };

    let older = u32::try_from(new.len()).map_or(0, |new| examined.exists.saturating_sub(new));
    let expunged = expunged(server, older, cached).await?;
    let flags = match (known.highest_modseq, examined.state.highest_modseq) {
        (Some(before), Some(now)) if before == now => Vec::new(),
        (Some(before), Some(_)) => server.flags(first, last, Some(before)).await?,
        _ => server.flags(first, last, None).await?,
    };

    Ok(Changes {
        state: examined.state,
        exists: examined.exists,
        unread,
        replace: false,
        new,
        expunged,
        flags,
    })
}

/// The cached UIDs gone from the server. `older` counts the server's
/// Messages up to the newest cached one.
async fn expunged(
    server: &mut impl MailServer,
    older: u32,
    cached: &[u32],
) -> Result<Vec<u32>, Error> {
    let (first, last) = (cached[0], cached[cached.len() - 1]);
    // No UID is ever assigned inside the cached range, so when the server
    // holds as many Messages from `first` on as the Cache does, none is gone.
    if let Some(sequence) = server.sequence_number(first).await?
        && usize::try_from(older.saturating_sub(sequence - 1)) == Ok(cached.len())
    {
        return Ok(Vec::new());
    }

    let present = server.uids(first, last).await?;
    let gone: Vec<_> = cached
        .iter()
        .copied()
        .filter(|uid| !present.contains(uid))
        .collect();
    tracing::info!("{} Messages expunged", gone.len());
    Ok(gone)
}
