use async_imap::imap_proto::{Response, ResponseCode, rfc4315::UidSetMember};

use super::Moved;
use crate::domain::uid::Uid;

/// Reads into `moved` each move a `COPYUID` names (RFC 4315), sent untagged
/// for a `UID MOVE` and with the completion for a `UID COPY`; false for any
/// other response.
pub fn read(response: &Response<'_>, moved: &mut Vec<Moved>) -> bool {
    let (Response::Data { outcome, .. } | Response::Done { outcome, .. }) = response else {
        return false;
    };

    if let Some(ResponseCode::CopyUid(_, from, to)) = &outcome.code {
        moved.extend(
            uids(from)
                .zip(uids(to))
                .map(|(from, to)| Moved { from, to }),
        );
    }

    true
}

fn uids(set: &[UidSetMember]) -> impl Iterator<Item = Uid> + '_ {
    set.iter()
        .flat_map(|member| match member {
            UidSetMember::UidRange(range) => range.clone(),
            UidSetMember::Uid(uid) => *uid..=*uid,
        })
        .map(Uid::new)
}
