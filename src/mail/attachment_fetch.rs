use crate::{
    domain::message_ref::MessageRef, mail::body_fetch, mail::imap::MailServer, mail::sync::Error,
    mail::update::Update,
};

/// An attachment wanted, by its place among its Message's.
#[derive(Clone)]
pub struct AttachmentRequest {
    pub message: MessageRef,
    pub index: usize,
}

/// How fetching one attachment ended: its decoded bytes, else why they
/// will not come.
pub struct AttachmentFetched {
    pub message: MessageRef,
    pub index: usize,
    pub bytes: Result<Vec<u8>, String>,
}

/// Fetches the part `request` names and tells `send`; it is never cached
/// (docs/adr/0024-the-preview-fetches-text-parts.md). Only a failure other
/// than a refusal is returned.
pub async fn fetch(
    server: &mut impl MailServer,
    request: &AttachmentRequest,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let AttachmentRequest { message, index } = request;
    let index = *index;
    let bytes = if body_fetch::open(server, &message.folder).await? {
        match server.attachment(message.uid, index).await {
            Ok(Some(bytes)) => Ok(bytes),
            Ok(None) => Err("the server no longer has it".to_owned()),
            Err(error) => match error.refusal() {
                Some(reason) => Err(reason.to_owned()),
                None => return Err(error.into()),
            },
        }
    } else {
        Err("its Folder is gone".to_owned())
    };

    send(Update::Attachment(AttachmentFetched {
        message: message.clone(),
        index,
        bytes,
    }));
    Ok(())
}
