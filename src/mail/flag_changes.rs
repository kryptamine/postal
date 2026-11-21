use crate::{
    cache::Cache,
    domain::flag::Flag,
    domain::folder_name::FolderName,
    domain::uid::Uid,
    mail::imap::MailServer,
    mail::sync::{self, Error},
};

/// Stores every flag change the Cache holds for the server, selecting each
/// Folder it is in. Takes the Folder selected now and returns it, or `None`
/// once another was selected. A refused change is dropped and only logged.
pub async fn send(
    server: &mut impl MailServer,
    cache: &Cache,
    mut selected: Option<FolderName>,
) -> Result<Option<FolderName>, Error> {
    let mut reselected = false;
    for (folder, changes) in cache.pending_flags()? {
        if selected.as_ref() != Some(&folder) {
            // A failed SELECT leaves no Folder selected (RFC 3501 6.3.1).
            (selected, reselected) = (None, true);
            match sync::select(server, &folder).await {
                Ok(_) => selected = Some(folder.clone()),
                Err(Error::Gone(error)) => {
                    tracing::warn!("{folder} is gone, its flag changes with it: {error}");
                    cache.flags_sent(&folder, &changes)?;
                    continue;
                }
                Err(error) => return Err(error),
            }
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

    Ok(selected.filter(|_| !reselected))
}
