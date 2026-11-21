use std::sync::Arc;

use crate::{
    cache::Cache, domain::folder::Role, mail::imap::MailServer, mail::sync::Error,
    mail::update::Update,
};

/// The Folder with `role`, listing the Folders first when the Cache knows
/// none.
pub async fn find(
    server: &mut impl MailServer,
    cache: &Cache,
    role: Role,
    send: &impl Fn(Update),
) -> Result<Option<Arc<str>>, Error> {
    let with_role = |cache: &Cache| -> Result<Option<Arc<str>>, Error> {
        let folders = cache.folders()?;
        Ok(folders
            .into_iter()
            .find(|f| f.role == Some(role))
            .map(|f| f.name))
    };

    if let Some(found) = with_role(cache)? {
        return Ok(Some(found));
    }

    cache.list_folders(&server.folders().await?)?;
    send(Update::Folders);
    with_role(cache)
}
