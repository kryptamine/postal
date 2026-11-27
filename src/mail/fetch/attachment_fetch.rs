use crate::{
    domain::message_ref::MessageRef, mail::fetch::part_fetch, mail::fetch::unfetched::Unfetched,
    mail::imap::MailServer, mail::sync::Error, mail::update::Update,
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
    pub bytes: Result<Vec<u8>, Unfetched>,
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
    let uid = message.uid;
    let fetched = part_fetch::in_folder(server, &message.folder, async |server| {
        server.attachment(uid, index).await
    });

    let bytes = fetched
        .await?
        .and_then(|bytes| bytes.ok_or_else(|| Unfetched::Failed(part_fetch::NOT_HELD.to_owned())));

    send(Update::Attachment(AttachmentFetched {
        message: message.clone(),
        index,
        bytes,
    }));
    Ok(())
}
