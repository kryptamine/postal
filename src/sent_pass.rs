use std::sync::Arc;

use crate::{
    cache::{Cache, SentPass},
    folder::Role,
    imap::MailServer,
    role_folder,
    sync::{self, Error},
    uid::Uid,
    update::Update,
};

/// The newest Messages of Sent the first pass reads, and one after a
/// UIDVALIDITY change.
const WINDOW: u32 = 1000;

/// Learns Correspondents from the envelopes of the Folder with the Sent
/// role newer than the last pass read. Returns the Folder it examined;
/// `None` when none has the role or the server refused it.
pub async fn read(
    server: &mut impl MailServer,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<Option<Arc<str>>, Error> {
    let Some(folder) = role_folder::find(server, cache, Role::Sent, send).await? else {
        return Ok(None);
    };

    let examined = match sync::examine(server, &folder).await {
        Err(Error::Gone(error)) => {
            tracing::warn!("examining {folder} for Correspondents was refused: {error}");
            return Ok(None);
        }
        result => result?,
    };

    let uid_validity = examined.state.uid_validity;
    let last = cache
        .sent_pass()?
        .filter(|pass| *pass.folder == *folder && pass.uid_validity == uid_validity)
        .map(|pass| pass.last_uid);
    let mut read = match (last, examined.state.uid_next) {
        (Some(last), Some(next)) if next <= last.next() => Vec::new(),
        (Some(last), _) => server.envelopes_since(last.next()).await?,
        (None, _) if examined.exists == 0 => Vec::new(),
        (None, _) => server.newest_envelopes(examined.exists, WINDOW).await?,
    };

    // Each was sent by the Account, from whichever address: none of those
    // is a Correspondent, and all in `To` were written to.
    for (_, correspondence) in &mut read {
        correspondence.sent = true;
        correspondence.from.clear();
    }

    let last_uid = read.iter().map(|(uid, _)| *uid).max().max(last);
    let pass = SentPass {
        folder: folder.to_string(),
        uid_validity,
        last_uid: last_uid.unwrap_or(Uid::new(0)),
    };

    cache.write_sent_pass(&pass, &read)?;
    tracing::info!(
        "learnt Correspondents from {} Messages of {folder}",
        read.len()
    );
    Ok(Some(folder))
}
