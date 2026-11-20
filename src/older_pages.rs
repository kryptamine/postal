use crate::{
    backfill::Backfill, body_fetch::Request, folder::INBOX, inbox_view::InboxView, routes::Routes,
    update::Update,
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
    live: Vec<bool>,
    asked: bool,
}

impl OlderPages {
    pub fn new(requests: Routes<Request>, accounts: usize) -> Self {
        Self {
            requests,
            live: vec![false; accounts],
            asked: false,
        }
    }

    /// Asks for a page when the selection is near the end and one may come.
    /// Does not allocate unless it asks. A Search never asks: it matches
    /// only the Cache.
    pub fn want(&mut self, view: &InboxView) {
        let Some(list) = &view.list else {
            return;
        };

        let searching = view.search.is_some();
        if self.asked || searching || view.selected + NEAR_END < view.rows() {
            return;
        }

        // With no edge, every Account has shown all it holds (ADR 0014).
        let (account, folder) = if view.folder.is_all_inboxes() {
            let Some(account) = view.edge else {
                return;
            };

            (account, INBOX)
        } else {
            (view.folder.account, &*view.folder.name)
        };

        let live = self.live.get(account).copied().unwrap_or(false);
        let mine = list.messages.iter().filter(|m| m.account == account);
        let Some(oldest) = mine.map(|m| m.uid).min() else {
            return;
        };

        if !live || view.bottomed(account, folder, oldest) {
            return;
        }

        self.asked = true;
        self.requests
            .send(account, Request::Backfill(folder.into()));
    }

    /// Follows the fetch connection, and passes `update` on. A request in
    /// flight when the connection went down was dropped with it.
    pub fn notice(&mut self, (account, update): (usize, Update)) -> (usize, Update) {
        match &update {
            Update::BodyLink(live) => {
                if let Some(was) = self.live.get_mut(account) {
                    *was = *live;
                }

                self.asked = false;
            }
            Update::Backfill(Backfill::Paged { .. }) => self.asked = false,
            _ => {}
        }

        (account, update)
    }
}
