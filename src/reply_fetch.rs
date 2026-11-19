use std::sync::Arc;

use futures::channel::mpsc::UnboundedSender;

use crate::{
    awaited_reply::AwaitedReply,
    body_fetch::{Fetched, Request},
    status::Status,
    update::Update,
};

/// Asks the fetch connection for the body an `AwaitedReply` waits on, and
/// tells when it is in the Cache.
pub struct ReplyFetch {
    requests: UnboundedSender<Request>,
    /// Whether the fetch connection is up.
    live: bool,
}

impl ReplyFetch {
    pub const fn new(requests: UnboundedSender<Request>) -> Self {
        Self {
            requests,
            live: false,
        }
    }

    /// Asks now while the fetch connection is up, else once it is.
    pub fn ask(&self, awaited: &AwaitedReply) {
        let Some(folder) = &awaited.message.folder else {
            return;
        };

        if self.live {
            // The send fails only once the fetch connection has stopped.
            self.requests
                .unbounded_send(Request::Reply(Arc::clone(folder), awaited.message.uid))
                .ok();
        }
    }

    /// Follows the fetch connection, asking again once it is back: what was
    /// in flight went down with it. Says when `update` brings the body
    /// `awaited` waits on, or why it will not come.
    pub fn notice(
        &mut self,
        update: &Update,
        awaited: Option<&AwaitedReply>,
    ) -> Option<Result<(), String>> {
        let awaited = match update {
            Update::BodyLink(live) => {
                self.live = *live;
                if let Some(awaited) = awaited {
                    self.ask(awaited);
                }

                return None;
            }
            _ => awaited?,
        };

        let message = &awaited.message;
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
