use std::sync::Arc;

use crate::{
    cache::Cache,
    imap::MailServer,
    sync::{self, Error},
};

/// Stores every flag change the Cache holds for the server, selecting each
/// Folder it is in. Takes the Folder selected now and returns the one left
/// selected. A refused change is dropped and only logged.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn send(
    server: &mut impl MailServer,
    cache: &Cache,
    mut selected: Option<Arc<str>>,
) -> Result<Option<Arc<str>>, Error> {
    for (folder, changes) in cache.pending_flags()? {
        if selected.as_deref() != Some(&*folder) {
            // A failed SELECT leaves no Folder selected (RFC 3501 6.3.1).
            selected = None;
            match sync::select(server, &folder).await {
                Ok(_) => selected = Some(Arc::clone(&folder)),
                Err(Error::Gone(error)) => {
                    tracing::warn!("{folder} is gone, its flag changes with it: {error}");
                    cache.flags_sent(&folder, &changes)?;
                    continue;
                }
                Err(error) => return Err(error),
            }
        }

        for unread in [false, true] {
            let uids: Vec<u32> = changes
                .iter()
                .filter(|&&(_, u)| u == unread)
                .map(|&(uid, _)| uid)
                .collect();
            if uids.is_empty() {
                continue;
            }

            match server.store_unread(&uids, unread).await {
                Err(error) if error.refusal().is_some() => {
                    tracing::warn!("storing flags in {folder} was refused: {error}");
                }
                result => result?,
            }
        }

        cache.flags_sent(&folder, &changes)?;
    }

    Ok(selected)
}
