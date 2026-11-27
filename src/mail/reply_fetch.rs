use crate::{
    compose::awaited_reply::AwaitedReply,
    mail::body_fetch::{Fetched, Request},
    mail::update::Update,
};

/// What the fetch connection is asked for while `awaited` waits.
pub fn requests(awaited: &AwaitedReply) -> [Request; 1] {
    [Request::Awaited(awaited.missing.clone())]
}

/// Says when `update` of its Account brings the last body `awaited` waits
/// on, or why the answered one will not come.
pub fn notice(update: &Update, awaited: &mut AwaitedReply) -> Option<Result<(), String>> {
    let message = &awaited.message;
    let (folder, uids) = match update {
        Update::Bodies(folder, Fetched::Stored(uids)) => (folder, uids),
        Update::Bodies(folder, Fetched::Refused { uids, reason }) => {
            if message.folder.as_ref() == Some(folder) && uids.contains(&message.uid) {
                return Some(Err(reason.clone()));
            }

            (folder, uids)
        }
        _ => return None,
    };

    let before = awaited.missing.len();
    awaited
        .missing
        .retain(|m| m.folder != *folder || !uids.contains(&m.uid));
    (awaited.missing.is_empty() && before > 0).then_some(Ok(()))
}
