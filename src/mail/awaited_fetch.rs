use crate::{
    compose::awaited::Awaited, domain::per_account::AccountId, mail::body_fetch::Standing,
    mail::forward_fetch, mail::reply_fetch, mail::status::Status, mail::update::Update,
};

/// What the fetch connection is asked for while `awaited` waits.
pub fn request(awaited: &Awaited) -> Standing {
    match awaited {
        Awaited::Reply(reply) => reply_fetch::request(reply),
        Awaited::Forward(forward) => forward_fetch::request(forward),
    }
}

/// Says when `update` of `account` brings the last of what `awaited` waits
/// on, or why some of it will not come.
pub fn notice(
    account: AccountId,
    update: &Update,
    awaited: &mut Awaited,
) -> Option<Result<(), String>> {
    if awaited.message().account != account {
        return None;
    }

    if matches!(update, Update::Status(Status::SignedOut)) {
        return Some(Err("signed out".to_owned()));
    }

    match awaited {
        Awaited::Reply(reply) => reply_fetch::notice(update, reply),
        Awaited::Forward(forward) => forward_fetch::notice(update, forward),
    }
}
