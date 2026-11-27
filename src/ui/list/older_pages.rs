use crate::{
    mail::backfill::Backfill, mail::body_fetch::Standing, mail::fetch_requests::FetchRequests,
    mail::update::Update, ui::list::inbox_view::InboxView,
};

/// Rows from the end of the Message List at which the next page is asked
/// for, so it is mostly in before the selection gets there.
const NEAR_END: usize = 50;

/// Asks the fetch connection for the next page of older Messages as the
/// selection nears the end of the Message List, one page at a time; in All
/// Inboxes, for the Account whose cache edge ends it.
#[derive(Default)]
pub struct OlderPages {
    asked: bool,
}

impl OlderPages {
    /// Asks for a page when the selection is near the end and one may come.
    /// Does not allocate unless it asks. A Search never asks: it matches
    /// only the Cache.
    pub fn want(&mut self, view: &InboxView, fetch: &FetchRequests) {
        let searching = view.search.is_some();
        if view.list.is_none() || self.asked || searching || view.selected + NEAR_END < view.rows()
        {
            return;
        }

        // With no edge, every Account has shown all it holds (ADR 0014).
        let Some((account, folder)) = view.edge.and_then(|a| Some((a, view.folder.of(a)?))) else {
            return;
        };

        fetch.send(account, Standing::Backfill(folder.clone()));
        self.asked = true;
    }

    /// Clears the in-flight mark once a page comes.
    pub const fn notice(&mut self, update: &Update) {
        if matches!(update, Update::Backfill(Backfill::Paged { .. })) {
            self.asked = false;
        }
    }
}
