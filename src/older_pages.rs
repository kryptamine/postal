use std::sync::Arc;

use futures::channel::mpsc::UnboundedSender;

use crate::{backfill::Backfill, body_fetch::Request, inbox_view::InboxView, update::Update};

/// Rows from the end of the Message List at which the next page is asked
/// for, so it is mostly in before the selection gets there.
const NEAR_END: usize = 50;

/// Asks the fetch connection for the next page of older Messages as the
/// selection nears the end of the Message List, one page at a time.
pub struct OlderPages {
    requests: UnboundedSender<Request>,
    /// Whether the fetch connection is up.
    live: bool,
    asked: bool,
    /// A Folder and its oldest cached UID, when the server had nothing
    /// older for it.
    nothing_below: Option<(Arc<str>, u32)>,
}

impl OlderPages {
    pub const fn new(requests: UnboundedSender<Request>) -> Self {
        Self {
            requests,
            live: false,
            asked: false,
            nothing_below: None,
        }
    }

    /// Asks for a page when the selection is near the end and one may come.
    /// Does not allocate unless it asks. A Search never asks: it matches
    /// only the Cache.
    pub fn want(&mut self, view: &InboxView) {
        let Some(messages) = view.messages.as_deref() else {
            return;
        };

        let searching = view.search.is_some();
        if !self.live || self.asked || searching || view.selected + NEAR_END < view.rows() {
            return;
        }

        let Some(oldest) = messages.iter().map(|m| m.uid).min() else {
            return;
        };

        let folder = &view.folder.name;
        if self
            .nothing_below
            .as_ref()
            .is_some_and(|(nothing, below)| nothing == folder && *below == oldest)
        {
            return;
        }

        self.asked = true;
        // The send fails only once the fetch connection has stopped.
        let _ = self
            .requests
            .unbounded_send(Request::Backfill(Arc::clone(folder)));
    }

    /// Follows the fetch connection, and passes `update` on. A request in
    /// flight when the connection went down was dropped with it.
    pub fn notice(&mut self, update: Update) -> Update {
        match &update {
            Update::BodyLink(live) => {
                self.live = *live;
                self.asked = false;
            }
            Update::Backfill(Backfill::Paged {
                folder,
                nothing_below,
            }) => {
                self.asked = false;
                self.nothing_below = nothing_below.map(|uid| (Arc::clone(folder), uid));
            }
            _ => {}
        }

        update
    }
}
