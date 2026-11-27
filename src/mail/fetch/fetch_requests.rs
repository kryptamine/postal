use crate::{
    domain::per_account::{AccountId, PerAccount},
    mail::fetch::body_fetch::{OneShot, Request, Standing},
    mail::routes::Routes,
    mail::status::Status,
    mail::update::{FetchLink, Update},
};

/// Routes every fetch request of the session, and tracks each Account's
/// fetch link.
pub struct FetchRequests {
    requests: Routes<Request>,
    links: PerAccount<FetchLink>,
}

impl FetchRequests {
    pub fn new(requests: Routes<Request>, statuses: &PerAccount<Status>) -> Self {
        let links = statuses.map(|status| {
            if *status == Status::SignedOut {
                FetchLink::Down
            } else {
                FetchLink::Starting
            }
        });

        Self { requests, links }
    }

    pub fn link(&self, account: AccountId) -> FetchLink {
        self.links[account]
    }

    /// Sends `request` to `account`, served once its fetch connection is up.
    pub fn send(&self, account: AccountId, request: Standing) {
        self.requests.send(account, Request::Standing(request));
    }

    /// Sends `request` while the fetch connection of `account` is live, and
    /// says whether it did.
    pub fn send_if_live(&self, account: AccountId, request: OneShot) -> bool {
        if self.links[account] != FetchLink::Live {
            return false;
        }

        self.requests.send(account, Request::OneShot(request));
        true
    }

    /// Follows the fetch connection of `account` through `update`.
    pub fn notice(&mut self, account: AccountId, update: &Update) {
        if let Update::FetchLink(link) = update {
            self.links[account] = *link;
        }
    }
}
