use crate::{
    cache::{self, Cache, Changes, Reported},
    domain::folder_name::FolderName,
    domain::sync_state::SyncState,
    domain::uid::Uid,
    mail::imap::{self, Examined, MailServer, Mode},
    mail::outage::Outage,
    mail::status::Status,
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

/// Opens `folder` as `mode` unless the open Folder covers it; `Gone` when refused.
pub async fn open(
    server: &mut impl MailServer,
    folder: &FolderName,
    mode: Mode,
) -> Result<(), Error> {
    server.open(folder, mode).await.map_err(gone)
}

/// Opens `folder` as `mode` and returns what it saw; `Gone` when refused.
pub async fn reopen(
    server: &mut impl MailServer,
    folder: &FolderName,
    mode: Mode,
) -> Result<Examined, Error> {
    server.reopen(folder, mode).await.map_err(gone)
}

/// A refusal to open a Folder means the server no longer has it.
fn gone(error: imap::Error) -> Error {
    if error.refusal().is_some() {
        Error::Gone(error)
    } else {
        Error::Imap(error)
    }
}

/// Brings the Cache up to the server's `folder`, opened as `mode`, in one
/// transaction, so the Message List never shows a half-applied resync.
/// Returns what its open saw and what it wrote; `report` follows the first
/// fetch, then says `Live`.
pub async fn resync(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &FolderName,
    mode: Mode,
    report: &dyn Fn(Status),
) -> Result<(Examined, Changes), Error> {
    let examined = reopen(server, folder, mode).await?;
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

            refetch(server, folder, &examined, unread, report).await?
        }
    };

    cache.apply(folder, &changes)?;
    report(Status::Live);
    Ok((examined, changes))
}

async fn refetch(
    server: &mut impl MailServer,
    folder: &FolderName,
    examined: &Examined,
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

    let reported = Reported::new(examined.state, examined.exists, unread);
    Ok(Changes::replacing(reported, new))
}

/// New mail by UID range, expunges over the cached range, then flags.
/// `cached` is ascending and not empty.
async fn update(
    server: &mut impl MailServer,
    examined: &Examined,
    known: SyncState,
    cached: &[Uid],
    unread: usize,
) -> Result<Changes, Error> {
    let (first, last) = (cached[0], cached[cached.len() - 1]);
    let new = if examined.state.uid_next.is_some() && examined.state.uid_next == known.uid_next {
        Vec::new()
    } else {
        server.since_uid(last.next()).await?
    };

    let older = u32::try_from(new.len()).map_or(0, |new| examined.exists.saturating_sub(new));
    let expunged = expunged(server, older, cached).await?;
    let flags = match (known.highest_modseq, examined.state.highest_modseq) {
        (Some(before), Some(now)) if before == now => Vec::new(),
        (Some(before), Some(_)) => server.flags(first, last, Some(before)).await?,
        _ => server.flags(first, last, None).await?,
    };

    let reported = Reported::new(examined.state, examined.exists, unread);
    Ok(Changes::update(reported, new, expunged, flags))
}

/// The cached UIDs gone from the server. `older` counts the server's
/// Messages up to the newest cached one.
async fn expunged(
    server: &mut impl MailServer,
    older: u32,
    cached: &[Uid],
) -> Result<Vec<Uid>, Error> {
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
