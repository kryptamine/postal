use crate::{
    domain::inline_image::InlineImage, domain::message_ref::MessageRef, mail::body_fetch,
    mail::imap::MailServer, mail::sync::Error, mail::unfetched::Unfetched, mail::update::Update,
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
    server: &mut impl MailServer,
    request: &InlineRequest,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let InlineRequest { message, ids } = request;
    let images = if body_fetch::open(server, &message.folder).await? {
        match server.inline_images(message.uid, ids).await {
            Ok(images) => Ok(images),
            Err(error) => match error.refusal() {
                Some(reason) => Err(Unfetched::Failed(reason.to_owned())),
                None => return Err(error.into()),
            },
        }
    } else {
        Err(Unfetched::Failed("its Folder is gone".to_owned()))
    };

    send(Update::Inline(InlineFetched {
        message: message.clone(),
        images,
    }));
    Ok(())
}
