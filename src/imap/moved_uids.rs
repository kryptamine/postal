use async_imap::{
    Session,
    imap_proto::{Response, ResponseCode, rfc4315::UidSetMember},
};

use super::{Error, Moved, connect::Transport, pushed::Pushed, tagged};
use crate::uid::Uid;

/// Runs `command`, a `UID MOVE` or a `UID COPY`, and returns each move its
/// `COPYUID` named (RFC 4315), sent untagged for a move and with the
/// completion for a copy. The `EXPUNGE`s on the way go to `pushed`.
pub async fn run(
    session: &mut Session<Box<dyn Transport>>,
    command: String,
    pushed: &mut Vec<Pushed>,
) -> Result<Vec<Moved>, Error> {
    let mut pairs = Vec::new();
    tagged::run(session, command, |response| match response {
        Response::Data { outcome, .. } | Response::Done { outcome, .. } => {
            if let Some(ResponseCode::CopyUid(_, from, to)) = &outcome.code {
                pairs.extend(
                    uids(from)
                        .zip(uids(to))
                        .map(|(from, to)| Moved { from, to }),
                );
            }
        }
        _ => pushed.extend(Pushed::from_response(response)),
    })
    .await?;
    Ok(pairs)
}

fn uids(set: &[UidSetMember]) -> impl Iterator<Item = Uid> + '_ {
    set.iter()
        .flat_map(|member| match member {
            UidSetMember::UidRange(range) => range.clone(),
            UidSetMember::Uid(uid) => *uid..=*uid,
        })
        .map(Uid::new)
}
