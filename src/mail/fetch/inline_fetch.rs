use crate::{
    domain::inline_image::InlineImage, domain::message_ref::MessageRef, mail::fetch::part_fetch,
    mail::fetch::unfetched::Unfetched, mail::imap::ImapSession, mail::sync::Error,
    mail::update::Update,
};

/// The parts a Message's HTML shows by `cid:`, by their `Content-ID`.
pub struct InlineRequest {
    pub message: MessageRef,
    pub ids: Vec<String>,
}

/// How fetching a Message's inline images ended: those found, else why
/// they will not come.
pub struct InlineFetched {
    pub message: MessageRef,
    pub images: Result<Vec<InlineImage>, Unfetched>,
}

/// Fetches what `request` names and tells `send`; like an attachment, it
/// is never cached. Only a failure other than a refusal is returned.
pub async fn fetch(
    server: &mut ImapSession,
    request: &InlineRequest,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let InlineRequest { message, ids } = request;
    let fetched = part_fetch::in_folder(server, &message.folder, async |server| {
        server.inline_images(message.uid, ids).await
    });

    let images = fetched.await?;

    send(Update::Inline(InlineFetched {
        message: message.clone(),
        images,
    }));
    Ok(())
}
