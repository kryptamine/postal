use crate::{
    domain::folder_name::FolderName, mail::body_fetch, mail::imap, mail::imap::MailServer,
    mail::sync::Error, mail::unfetched::Unfetched,
};

pub const NOT_HELD: &str = "the server no longer has it";
pub const FOLDER_GONE: &str = "its Folder is gone";

/// `fetch` once `folder` is open. A refusal, or the Folder gone, is why the
/// part will not come; only another failure is returned.
pub async fn in_folder<S: MailServer, T>(
    server: &mut S,
    folder: &FolderName,
    fetch: impl AsyncFnOnce(&mut S) -> Result<T, imap::Error>,
) -> Result<Result<T, Unfetched>, Error> {
    if !body_fetch::open(server, folder).await? {
        return Ok(Err(Unfetched::Failed(FOLDER_GONE.to_owned())));
    }

    let error = match fetch(server).await {
        Ok(part) => return Ok(Ok(part)),
        Err(error) => error,
    };

    let Some(reason) = error.refusal() else {
        return Err(error.into());
    };

    Ok(Err(Unfetched::Failed(reason.to_owned())))
}
