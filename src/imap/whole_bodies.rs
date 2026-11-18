use async_imap::{
    Session,
    imap_proto::{AttributeValue, Response},
};

use super::{Error, connect::Transport, tagged};

/// `UID FETCH uids (UID BODY.PEEK[])`, each `(uid, raw)` it answered. What
/// else the server says on the way is dropped.
pub async fn fetch(
    session: &mut Session<Box<dyn Transport>>,
    uids: &str,
) -> Result<Vec<(u32, Vec<u8>)>, Error> {
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

fn body(attributes: &[AttributeValue<'_>]) -> Option<(u32, Vec<u8>)> {
    let uid = attributes.iter().find_map(|a| match a {
        AttributeValue::Uid(uid) => Some(*uid),
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
