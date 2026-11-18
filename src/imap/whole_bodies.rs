use async_imap::{
    Session,
    imap_proto::{AttributeValue, Response, Status},
};

use super::{Error, connect::Transport};

/// `UID FETCH uids (UID BODY.PEEK[])`, each `(uid, raw)` it answered. Read
/// by hand, as async-imap's own fetch ends quietly on a `NO` and on a closed
/// connection alike. What else the server says on the way is dropped.
pub async fn fetch(
    session: &mut Session<Box<dyn Transport>>,
    uids: &str,
) -> Result<Vec<(u32, Vec<u8>)>, Error> {
    let tag = session
        .run_command(format!("UID FETCH {uids} (UID BODY.PEEK[])"))
        .await?;
    let mut bodies = Vec::new();
    loop {
        let response = session
            .read_response()
            .await
            .map_err(|e| Error::Imap(e.into()))?
            .ok_or(Error::Closed)?;
        match response.parsed() {
            Response::Fetch(_, attributes) => bodies.extend(body(attributes)),
            Response::Done {
                tag: done,
                status,
                outcome,
            } if *done == tag => {
                // Worded as async-imap words its own, which `Error` reads.
                let text = format!("code: {:?}, info: {:?}", outcome.code, outcome.information);
                return match status {
                    Status::Ok => Ok(bodies),
                    Status::No => Err(Error::Imap(async_imap::error::Error::No(text))),
                    _ => Err(Error::Imap(async_imap::error::Error::Bad(text))),
                };
            }
            _ => {}
        }
    }
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
