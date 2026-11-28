use crate::{
    cache::Cache, domain::folder_name::FolderName, domain::message_ref::MessageRef,
    domain::uid::Uid, mail::fetch::attachment_fetch::AttachmentRequest,
    mail::fetch::inline_fetch::InlineRequest, mail::imap::ImapSession, mail::sync::Error,
    mail::update::Update,
};

/// Work for the fetch connection, as its channel carries it.
pub enum Request {
    Standing(Standing),
    OneShot(OneShot),
}

/// Work kept across reconnects until served. Each replaces the one of its
/// kind before it.
pub enum Standing {
    Bodies(BodyRequest),
    /// The page of Messages of this Folder older than every cached one.
    Backfill(FolderName),
    /// What a Reply or a Forward waits on: served first, and never passed
    /// over for a later request.
    Awaited(AwaitedRequest),
}

/// Work served once, in turn. When its connection drops, it is answered
/// with an offline failure.
pub enum OneShot {
    Attachment(AttachmentRequest),
    /// The inline images of a Message opened in the browser.
    Inline(InlineRequest),
}

pub struct AwaitedRequest {
    pub bodies: Vec<MessageRef>,
    pub attachments: Vec<AttachmentRequest>,
}

/// Bodies the Preview needs, by UID in `folder`. Each request replaces the
/// one before it, so it names every body still missing. The selected one
/// comes alone, so it does not wait on its neighbours.
pub struct BodyRequest {
    pub folder: FolderName,
    pub selected: Option<Uid>,
    pub neighbours: Vec<Uid>,
}

/// How one fetch of bodies ended.
pub enum Fetched {
    /// These UIDs' bodies are now in the Cache.
    Stored(Vec<Uid>),
    /// The server answered `NO` for these UIDs, with this reason.
    Refused { uids: Vec<Uid>, reason: String },
}

impl Fetched {
    fn uids(&self) -> &[Uid] {
        match self {
            Self::Stored(uids) | Self::Refused { uids, .. } => uids,
        }
    }
}

/// Fetches `uids` and says how it went. A `NO` names no UID: they are asked
/// again one by one, so a single refused body does not take its batch with it.
/// Returns the UIDs the server answered for, stored or refused.
pub async fn fetch_each(
    server: &mut ImapSession,
    cache: &Cache,
    folder: &FolderName,
    uids: Vec<Uid>,
    send: &impl Fn(Update),
) -> Result<Vec<Uid>, Error> {
    let mut answered = Vec::new();
    match fetch(server, cache, folder, uids).await? {
        Fetched::Refused { uids, .. } if uids.len() > 1 => {
            for uid in uids {
                let fetched = fetch(server, cache, folder, vec![uid]).await?;
                answered.extend_from_slice(fetched.uids());
                send(Update::Bodies(folder.clone(), fetched));
            }
        }
        done => {
            answered.extend_from_slice(done.uids());
            send(Update::Bodies(folder.clone(), done));
        }
    }

    Ok(answered)
}

async fn fetch(
    server: &mut ImapSession,
    cache: &Cache,
    folder: &FolderName,
    uids: Vec<Uid>,
) -> Result<Fetched, Error> {
    let bodies = match server.bodies(&uids).await {
        Ok(bodies) => bodies,
        Err(error) => {
            let Some(reason) = error.refusal() else {
                return Err(error.into());
            };

            tracing::warn!("fetching the bodies of {uids:?} was refused: {error}");
            let reason = reason.to_owned();
            return Ok(Fetched::Refused { uids, reason });
        }
    };

    cache.write_bodies(folder, &bodies)?;
    tracing::debug!("fetched {} of {} bodies", bodies.len(), uids.len());
    Ok(Fetched::Stored(
        bodies.into_iter().map(|(uid, _)| uid).collect(),
    ))
}
