use std::sync::Arc;

use crate::{
    awaited_reply::AwaitedReply,
    body_fetch::{Fetched, Request},
    routes::Routes,
    status::Status,
    update::Update,
};

/// Asks the fetch connection for the body an `AwaitedReply` waits on, and
/// tells when it is in the Cache.
pub struct ReplyFetch {
    requests: Routes<Request>,
    /// Whether each Account's fetch connection is up.
    live: Vec<bool>,
}

impl ReplyFetch {
    pub fn new(requests: Routes<Request>, accounts: usize) -> Self {
        Self {
            requests,
            live: vec![false; accounts],
        }
    }

    /// Asks now while the fetch connection is up, else once it is.
    pub fn ask(&self, awaited: &AwaitedReply) {
        let Some(folder) = &awaited.message.folder else {
            return;
        };

        let account = awaited.message.account;
        if self.live.get(account).copied().unwrap_or(false) {
            let request = Request::Reply(Arc::clone(folder), awaited.message.uid);
            self.requests.send(account, request);
        }
    }

    /// Follows the fetch connection, asking again once it is back: what was
    /// in flight went down with it. Says when `update` brings the body
    /// `awaited` waits on, or why it will not come.
    pub fn notice(
        &mut self,
        (account, update): (usize, &Update),
        awaited: Option<&AwaitedReply>,
    ) -> Option<Result<(), String>> {
        let awaited = match update {
            Update::BodyLink(live) => {
                if let Some(was) = self.live.get_mut(account) {
                    *was = *live;
                }

                if let Some(awaited) = awaited {
                    self.ask(awaited);
                }

                return None;
            }
            _ => awaited?,
        };

        let message = &awaited.message;
        if message.account != account {
            return None;
        }

        match update {
            Update::Status(Status::SignedOut) => Some(Err("signed out".to_owned())),
            Update::Bodies(folder, fetched) if message.folder.as_ref() == Some(folder) => {
                match fetched {
                    Fetched::Stored(uids) if uids.contains(&message.uid) => Some(Ok(())),
                    Fetched::Refused { uids, reason } if uids.contains(&message.uid) => {
                        Some(Err(reason.clone()))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}
