use crate::{
    domain::folder_name::FolderName,
    mail::imap::{ImapSession, Mode},
    mail::sync::{self, Error},
};

/// Examines `folder` unless it is open already; false when the server no
/// longer has it.
pub async fn open(server: &mut ImapSession, folder: &FolderName) -> Result<bool, Error> {
    match sync::open(server, folder, Mode::Examine).await {
        Ok(()) => Ok(true),
        Err(Error::Gone(error)) => {
            tracing::warn!("{folder} is gone: {error}");
            Ok(false)
        }
        Err(error) => Err(error),
    }
}
