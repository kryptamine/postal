use crate::{
    domain::per_account::{AccountId, PerAccount},
    mail::body_fetch::Request,
    mail::routes::Routes,
    mail::status::Status,
    mail::update::{FetchLink, Update},
};

/// Routes every fetch request of the session, sending one only while its
/// Account's fetch connection is live.
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

    /// Sends `requests` while the fetch connection of `account` is live, and
    /// says whether it did; otherwise none is taken from the iterator.
    pub fn send(&self, account: AccountId, requests: impl IntoIterator<Item = Request>) -> bool {
        if self.links[account] != FetchLink::Live {
            return false;
        }

        for request in requests {
            self.requests.send(account, request);
        }

        true
    }

    /// Follows the fetch connection of `account` through `update`.
    pub fn notice(&mut self, account: AccountId, update: &Update) {
        if let Update::FetchLink(link) = update {
            self.links[account] = *link;
        }
    }
}
