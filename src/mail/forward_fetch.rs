use crate::{
    compose::awaited_forward::AwaitedForward,
    domain::per_account::{AccountId, PerAccount},
    mail::attachment_fetch::AttachmentRequest,
    mail::body_fetch::{Fetched, Request},
    mail::routes::Routes,
    mail::status::Status,
    mail::update::{BodyLink, Update},
};

/// Asks the fetch connection for what an `AwaitedForward` waits on, and
/// tells when all of it is in.
pub struct ForwardFetch {
    requests: Routes<Request>,
    /// Whether each Account's fetch connection is up.
    live: PerAccount<bool>,
}

impl ForwardFetch {
    pub const fn new(requests: Routes<Request>, live: PerAccount<bool>) -> Self {
        Self { requests, live }
    }

    /// Asks now for what is still missing while the fetch connection is
    /// up, else once it is.
    pub fn ask(&self, awaited: &AwaitedForward) {
        let account = awaited.message.account;
        if !self.live[account] {
            return;
        }

        if awaited.body_missing {
            let body = Request::Reply(vec![awaited.on_server.clone()]);
            self.requests.send(account, body);
        }

        for (index, _) in awaited
            .attachments
            .iter()
            .enumerate()
            .filter(|(_, a)| a.is_none())
        {
            let request = AttachmentRequest {
                message: awaited.on_server.clone(),
                index,
            };

            self.requests.send(account, Request::Attachment(request));
        }
    }

    /// Follows the fetch connection, asking again once it is back: what was
    /// in flight went down with it. Says when `update` brings the last part
    /// `awaited` waits on, or why one will not come.
    pub fn notice(
        &mut self,
        account: AccountId,
        update: &Update,
        awaited: Option<&mut AwaitedForward>,
    ) -> Option<Result<(), String>> {
        if let Update::BodyLink(link) = update {
            self.live[account] = *link == BodyLink::Up;
            if let Some(awaited) = awaited {
                self.ask(awaited);
            }

            return None;
        }

        let awaited = awaited.filter(|a| a.message.account == account)?;
        let ours = &awaited.on_server;
        match update {
            Update::Status(Status::SignedOut) => return Some(Err("signed out".to_owned())),
            Update::Bodies(folder, fetched) if *folder == ours.folder => match fetched {
                Fetched::Stored(uids) if uids.contains(&ours.uid) => awaited.body_missing = false,
                Fetched::Refused { uids, reason } if uids.contains(&ours.uid) => {
                    return Some(Err(reason.clone()));
                }
                _ => return None,
            },
            Update::Attachment(fetched) if fetched.message == *ours => {
                let slot = awaited.attachments.get_mut(fetched.index)?;
                match &fetched.bytes {
                    Ok(bytes) => *slot = Some(bytes.clone()),
                    Err(reason) => return Some(Err(reason.clone())),
                }
            }
            _ => return None,
        }

        awaited.complete().then_some(Ok(()))
    }
}
