use crate::{
    awaited_reply::AwaitedReply,
    body_fetch::{Fetched, Request},
    per_account::{AccountId, PerAccount},
    routes::Routes,
    status::Status,
    update::Update,
};

/// Asks the fetch connection for the bodies an `AwaitedReply` waits on,
/// and tells when they are in the Cache.
pub struct ReplyFetch {
    requests: Routes<Request>,
    /// Whether each Account's fetch connection is up.
    live: PerAccount<bool>,
}

impl ReplyFetch {
    pub const fn new(requests: Routes<Request>, live: PerAccount<bool>) -> Self {
        Self { requests, live }
    }

    /// Asks now while the fetch connection is up, else once it is.
    pub fn ask(&self, awaited: &AwaitedReply) {
        let account = awaited.message.account;
        if self.live[account] {
            let request = Request::Reply(awaited.missing.clone());
            self.requests.send(account, request);
        }
    }

    /// Follows the fetch connection, asking again once it is back: what was
    /// in flight went down with it. Says when `update` brings the last body
    /// `awaited` waits on, or why the answered one will not come.
    pub fn notice(
        &mut self,
        (account, update): (AccountId, &Update),
        awaited: Option<&mut AwaitedReply>,
    ) -> Option<Result<(), String>> {
        let awaited = match update {
            Update::BodyLink(live) => {
                self.live[account] = *live;

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

        let (folder, uids) = match update {
            Update::Status(Status::SignedOut) => return Some(Err("signed out".to_owned())),
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
}
