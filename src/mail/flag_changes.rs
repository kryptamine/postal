use crate::{
    cache::Cache,
    domain::flag::Flag,
    domain::uid::Uid,
    mail::imap::{MailServer, Mode},
    mail::sync::{self, Error},
};

/// Stores every flag change the Cache holds for the server, selecting each
/// Folder it is in. A refused change is dropped and only logged.
pub async fn send(server: &mut impl MailServer, cache: &Cache) -> Result<(), Error> {
    for (folder, changes) in cache.pending_flags()? {
        match sync::open(server, &folder, Mode::Select).await {
            Err(Error::Gone(error)) => {
                tracing::warn!("{folder} is gone, its flag changes with it: {error}");
                cache.flags_sent(&folder, &changes)?;
                continue;
            }
            result => result?,
        }

        for flag in [Flag::Seen, Flag::Answered] {
            for added in [true, false] {
                let uids: Vec<Uid> = changes
                    .iter()
                    .filter(|c| c.flag == flag && c.added == added)
                    .map(|c| c.uid)
                    .collect();
                if uids.is_empty() {
                    continue;
                }

                match server.store(&uids, flag, added).await {
                    Err(error) if error.refusal().is_some() => {
                        tracing::warn!("storing flags in {folder} was refused: {error}");
                    }
                    result => result?,
                }
            }
        }

        cache.flags_sent(&folder, &changes)?;
    }

    Ok(())
}
