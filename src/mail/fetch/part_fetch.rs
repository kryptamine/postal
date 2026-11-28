use crate::{
    domain::folder_name::FolderName, mail::fetch::examine, mail::fetch::unfetched::Unfetched,
    mail::imap, mail::imap::ImapSession, mail::sync::Error,
};

pub const NOT_HELD: &str = "the server no longer has it";
pub const FOLDER_GONE: &str = "its Folder is gone";

/// `fetch` once `folder` is open. A refusal, or the Folder gone, is why the
/// part will not come; only another failure is returned.
pub async fn in_folder<T>(
    server: &mut ImapSession,
    folder: &FolderName,
    fetch: impl AsyncFnOnce(&mut ImapSession) -> Result<T, imap::Error>,
) -> Result<Result<T, Unfetched>, Error> {
    if !examine::open(server, folder).await? {
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
