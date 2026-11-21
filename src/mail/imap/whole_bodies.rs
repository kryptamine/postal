use async_imap::{
    Session,
    imap_proto::{AttributeValue, Response},
};

use super::{Error, connect::Transport, tagged};
use crate::domain::uid::Uid;

/// `UID FETCH uids (UID BODY.PEEK[])`, each `(uid, raw)` it answered. What
/// else the server says on the way is dropped.
pub async fn fetch(
    session: &mut Session<Box<dyn Transport>>,
    uids: &str,
) -> Result<Vec<(Uid, Vec<u8>)>, Error> {
    let mut bodies = Vec::new();
    let command = format!("UID FETCH {uids} (UID BODY.PEEK[])");
    tagged::run(session, command, |response| {
        if let Response::Fetch(_, attributes) = response {
            bodies.extend(body(attributes));
        }
    })
    .await?;
    Ok(bodies)
}

fn body(attributes: &[AttributeValue<'_>]) -> Option<(Uid, Vec<u8>)> {
    let uid = attributes.iter().find_map(|a| match a {
        AttributeValue::Uid(uid) => Some(Uid::new(*uid)),
        _ => None,
    })?;
    let raw = attributes.iter().find_map(|a| match a {
        AttributeValue::BodySection {
            section: None,
            data: Some(data),
            ..
        } => Some(data.to_vec()),
        _ => None,
    })?;
    Some((uid, raw))
}
