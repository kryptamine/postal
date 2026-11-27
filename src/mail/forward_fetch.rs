use crate::{
    compose::awaited_forward::AwaitedForward,
    mail::attachment_fetch::AttachmentRequest,
    mail::body_fetch::{AwaitedRequest, Fetched, Standing},
    mail::update::Update,
};

/// What the fetch connection is asked for while `awaited` waits: the body
/// if the Cache lacks it, and each attachment not in yet.
pub fn request(awaited: &AwaitedForward) -> Standing {
    let ours = &awaited.on_server;
    let bodies = if awaited.body_missing {
        vec![ours.clone()]
    } else {
        Vec::new()
    };

    let attachments = (0..)
        .zip(&awaited.attachments)
        .filter(|(_, bytes)| bytes.is_none())
        .map(|(index, _)| AttachmentRequest {
            message: ours.clone(),
            index,
        })
        .collect();

    Standing::Awaited(AwaitedRequest {
        bodies,
        attachments,
    })
}

/// Says when `update` of its Account brings the last part `awaited` waits
/// on, or why one will not come.
pub fn notice(update: &Update, awaited: &mut AwaitedForward) -> Option<Result<(), String>> {
    let ours = &awaited.on_server;
    match update {
        Update::Bodies(folder, fetched) if *folder == ours.folder => match fetched {
            Fetched::Stored(uids) if uids.contains(&ours.uid) => awaited.body_missing = false,
            Fetched::Refused { uids, reason } if uids.contains(&ours.uid) => {
                return Some(Err(reason.clone()));
            }
            _ => return None,
        },
        Update::Attachment(fetched) if fetched.message == *ours => {
            let slot = awaited.attachments.get_mut(fetched.index)?;
            match &fetched.bytes {
                Ok(bytes) => *slot = Some(bytes.clone()),
                Err(reason) => return Some(Err(reason.clone())),
            }
        }
        _ => return None,
    }

    awaited.complete().then_some(Ok(()))
}
