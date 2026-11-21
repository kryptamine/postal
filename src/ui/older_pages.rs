use crate::{
    domain::per_account::{AccountId, PerAccount},
    mail::backfill::Backfill,
    mail::body_fetch::Request,
    mail::routes::Routes,
    mail::update::{BodyLink, Update},
    ui::inbox_view::InboxView,
};

/// Rows from the end of the Message List at which the next page is asked
/// for, so it is mostly in before the selection gets there.
const NEAR_END: usize = 50;

/// Asks the fetch connection for the next page of older Messages as the
/// selection nears the end of the Message List, one page at a time; in All
/// Inboxes, for the Account whose cache edge ends it.
pub struct OlderPages {
    requests: Routes<Request>,
    /// Whether each Account's fetch connection is up.
    live: PerAccount<bool>,
    asked: bool,
}

impl OlderPages {
    pub const fn new(requests: Routes<Request>, live: PerAccount<bool>) -> Self {
        Self {
            requests,
            live,
            asked: false,
        }
    }

    /// Asks for a page when the selection is near the end and one may come.
    /// Does not allocate unless it asks. A Search never asks: it matches
    /// only the Cache.
    pub fn want(&mut self, view: &InboxView) {
        let searching = view.search.is_some();
        if view.list.is_none() || self.asked || searching || view.selected + NEAR_END < view.rows()
        {
            return;
        }

        // With no edge, every Account has shown all it holds (ADR 0014).
        let Some((account, folder)) = view.edge.and_then(|a| Some((a, view.folder.of(a)?))) else {
            return;
        };

        if !self.live[account] {
            return;
        }

        self.asked = true;
        self.requests
            .send(account, Request::Backfill(folder.clone()));
    }

    /// Follows the fetch connection, and passes `update` on. A request in
    /// flight when the connection went down was dropped with it.
    pub fn notice(&mut self, (account, update): (AccountId, Update)) -> (AccountId, Update) {
        match &update {
            Update::BodyLink(link) => {
                self.live[account] = *link == BodyLink::Up;

                self.asked = false;
            }
            Update::Backfill(Backfill::Paged { .. }) => self.asked = false,
            _ => {}
        }

        (account, update)
    }
}
