use crate::{
    cache::Cache, domain::folder_name::FolderName, mail::imap::MailServer, mail::sync::Error,
    mail::update::Update,
};

/// Lists the Folders into the Cache, then counts the Messages of each but
/// `shown` by `STATUS`, telling `send` after every write so the counts show
/// as they come. Returns whether `shown` is still listed.
pub async fn refresh(
    server: &mut impl MailServer,
    cache: &Cache,
    shown: &FolderName,
    send: &impl Fn(Update),
) -> Result<bool, Error> {
    let listed = server.folders().await?;
    cache.list_folders(&listed)?;
    send(Update::Folders);
    // RFC 3501 advises against `STATUS` on the selected Folder: ADR 0008.
    for folder in listed.iter().filter(|f| f.name != *shown) {
        match server.counts_in(&folder.name).await {
            Ok((unread, total)) => cache.write_counts(&folder.name, unread, total)?,
            Err(error) if error.refusal().is_some() => {
                tracing::warn!(
                    "counting the Messages of {} was refused: {error}",
                    folder.name
                );
                continue;
            }
            Err(error) => return Err(error.into()),
        }

        send(Update::Folders);
    }

    Ok(listed.iter().any(|f| f.name == *shown))
}
