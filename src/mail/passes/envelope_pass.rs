use crate::{
    cache::{Cache, EnvelopePass},
    domain::correspondence::Correspondence,
    domain::folder::Role,
    domain::folder_name::FolderName,
    domain::folder_name::INBOX,
    domain::uid::Uid,
    mail::imap::{ImapSession, Mode},
    mail::role_folder,
    mail::sync::{self, Error},
    mail::update::Update,
};

/// Envelopes read per step, newest first, so a request waits on one alone.
const BATCH: u32 = 500;

type Envelopes = Vec<(Uid, Correspondence)>;

/// Learns Correspondents from one more batch of envelopes in All Mail on
/// Gmail, else the Inbox: newest first, then older ones, then those past
/// the last UID read. Returns whether older ones are still to read.
pub async fn step(
    server: &mut ImapSession,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<bool, Error> {
    let folder = target(server, cache, send).await?;
    // A fresh EXAMINE, for this moment's UIDNEXT and sequence numbers.
    let examined = match sync::reopen(server, &folder, Mode::Examine).await {
        Err(Error::Gone(error)) => {
            tracing::warn!("examining {folder} for Correspondents was refused: {error}");
            return Ok(false);
        }
        result => result?,
    };

    let uid_validity = examined.state.uid_validity;
    // The UID below UIDNEXT, so one expunged past the last read is not
    // asked for again on every step.
    let below_next = examined
        .state
        .uid_next
        .map(|next| Uid::new(next.get().saturating_sub(1)));
    let known = cache
        .envelope_pass()?
        .filter(|pass| pass.folder == *folder.as_str() && pass.uid_validity == uid_validity);
    let (read, last_uid, below) = match known {
        None => start(server, cache, &folder, examined.exists, below_next).await?,
        Some(pass) if below_next.is_some_and(|newest| newest > pass.last_uid) => {
            let read = server.envelopes_since(pass.last_uid.next()).await?;
            let newest = highest(&read).max(below_next);
            (read, newest, pass.below)
        }
        Some(EnvelopePass { below: None, .. }) => return Ok(false),
        Some(EnvelopePass {
            below: Some(below),
            last_uid,
            ..
        }) => {
            let before = sequence_number(server, below).await?;
            let (read, below) = older(server, before).await?;
            (read, Some(last_uid), below)
        }
    };

    let count = read.len();
    let pass = EnvelopePass {
        folder: folder.to_string(),
        uid_validity,
        last_uid: last_uid.unwrap_or(Uid::new(0)),
        below,
    };

    cache.write_envelope_pass(&pass, read)?;
    tracing::info!("learnt Correspondents from {count} Messages of {folder}");
    if below.is_none() {
        tracing::info!("read every envelope of {folder}");
    }

    Ok(below.is_some())
}

/// The first step in `folder`, which holds `exists` Messages: the newest
/// batch, else nothing in an Inbox whose newest the Cache holds already,
/// so the walk starts below them.
async fn start(
    server: &mut ImapSession,
    cache: &Cache,
    folder: &FolderName,
    exists: u32,
    below_next: Option<Uid>,
) -> Result<(Envelopes, Option<Uid>, Option<Uid>), Error> {
    let cached = cache.oldest_uid(folder)?;
    if let Some(oldest) = cached.filter(|_| folder.as_str() == INBOX) {
        return Ok((Vec::new(), below_next, Some(oldest)));
    }

    let (read, below) = older(server, exists.saturating_add(1)).await?;
    let newest = highest(&read);
    Ok((read, newest, below))
}

fn highest(read: &Envelopes) -> Option<Uid> {
    read.iter().map(|(uid, _)| *uid).max()
}

/// All Mail on Gmail, else the Inbox.
async fn target(
    server: &mut ImapSession,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<FolderName, Error> {
    let all = if server.gmail() {
        role_folder::find(server, cache, Role::All, send).await?
    } else {
        None
    };

    Ok(all.unwrap_or_else(|| FolderName::from(INBOX)))
}

/// The batch just before sequence number `before`, with the UID below
/// which more are left, if any.
async fn older(server: &mut ImapSession, before: u32) -> Result<(Envelopes, Option<Uid>), Error> {
    let read = server.envelopes_before(before, BATCH).await?;
    let oldest = read.iter().map(|(uid, _)| *uid).min();
    Ok((read, oldest.filter(|_| before.saturating_sub(BATCH) > 1)))
}

async fn sequence_number(server: &mut ImapSession, uid: Uid) -> Result<u32, Error> {
    if let Some(found) = server.sequence_number(uid).await? {
        return Ok(found);
    }

    // Expunged since it was read: its number is one past the count of
    // Messages older than it.
    let below = server
        .uids(Uid::new(1), Uid::new(uid.get().saturating_sub(1)))
        .await?;
    Ok(u32::try_from(below.len())
        .unwrap_or(u32::MAX)
        .saturating_add(1))
}
