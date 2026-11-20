use std::sync::Arc;

use crate::{
    backfill::Backfill, body_fetch::Request, inbox_view::InboxView, routes::Routes, update::Update,
};

/// Rows from the end of the Message List at which the next page is asked
/// for, so it is mostly in before the selection gets there.
const NEAR_END: usize = 50;

/// Asks the fetch connection for the next page of older Messages as the
/// selection nears the end of the Message List, one page at a time.
pub struct OlderPages {
    requests: Routes<Request>,
    /// Whether each Account's fetch connection is up.
    live: Vec<bool>,
    asked: bool,
    /// A Folder, of an Account, and its oldest cached UID, when the server
    /// had nothing older for it.
    nothing_below: Option<(usize, Arc<str>, u32)>,
}

impl OlderPages {
    pub fn new(requests: Routes<Request>, accounts: usize) -> Self {
        Self {
            requests,
            live: vec![false; accounts],
            asked: false,
            nothing_below: None,
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
        let account = view.folder.account;
        let live = self.live.get(account).copied().unwrap_or(false);
        if !live || self.asked || searching || view.selected + NEAR_END < view.rows() {
            return;
        }

        let Some(oldest) = list.messages.iter().map(|m| m.uid).min() else {
            return;
        };

        let folder = &view.folder.name;
        if self
            .nothing_below
            .as_ref()
            .is_some_and(|(at, nothing, below)| {
                *at == account && nothing == folder && *below == oldest
            })
        {
            return;
        }

        self.asked = true;
        self.requests
            .send(account, Request::Backfill(Arc::clone(folder)));
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
            Update::Backfill(Backfill::Paged {
                folder,
                nothing_below,
            }) => {
                self.asked = false;
                self.nothing_below = nothing_below.map(|uid| (account, Arc::clone(folder), uid));
            }
            _ => {}
        }

        (account, update)
    }
}
