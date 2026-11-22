use tokio::time::Instant;

use crate::{
    cache::Cache,
    compose::awaited_reply::AwaitedReply,
    domain::per_account::{AccountId, PerAccount},
    domain::thread::Key,
    mail::status::Status,
    ui::dwell::Dwell,
    ui::list::inbox_view::InboxView,
    ui::mode::Mode,
    ui::popup::layer::Layer,
    ui::popup::over::Over,
    ui::preview::link_hints::LinkHints,
    ui::showing::Showing,
    ui::toast::Toast,
    ui::undo::Undo,
};

/// What the main screen shows over and beside the panes.
#[derive(Default)]
pub struct Screen {
    /// Over everything, `over` included, taking every key while open.
    pub help: bool,
    pub toast: Option<Toast>,
    /// Each Account's sync connection's.
    pub statuses: PerAccount<Status>,
    /// A page of older Messages under way on the fetch connection.
    pub backfill: Option<Status>,
    pub showing: Showing,
    pub over: Over,
    pub dwell: Dwell,
    /// What an Undo may take back, the latest last.
    pub undo: Vec<Undo>,
    pub replying: Option<AwaitedReply>,
    /// The Messages in the Outbox not yet accepted.
    pub outbox: usize,
}

impl Screen {
    /// `rebuilt` when opening replaced the Cache; `failure` toasts what
    /// failed before it opened.
    pub fn opening(
        rebuilt: bool,
        statuses: PerAccount<Status>,
        failure: Option<&'static str>,
    ) -> Self {
        let toast = failure.map(Toast::error);
        Self {
            statuses,
            toast: toast.or_else(|| rebuilt.then(|| Toast::new("cache rebuilt"))),
            ..Self::default()
        }
    }

    pub fn list(&mut self) {
        self.showing = Showing::List;
    }

    /// Back to the list once the selection left `thread`, the Thread Reading
    /// was opened on; true when it did.
    pub fn follow(&mut self, view: &InboxView, thread: Option<Key>) -> bool {
        let left = self.showing.reading().is_some() && view.selected_key() != thread;
        if left {
            self.over.close::<LinkHints>();
            self.list();
        }

        left
    }

    /// What the keys act on, below the `?` popup: the top layer over the
    /// panes, else the panes.
    pub fn keys(&self) -> Mode {
        self.over
            .top()
            .and_then(Layer::mode)
            .unwrap_or_else(|| self.showing.mode())
    }

    /// The worst sync connection's Status, with its Account when there are
    /// several; while all are live, a Reply's fetch, else the backfill's.
    pub fn status(&self) -> (Status, Option<AccountId>) {
        let worst = self
            .statuses
            .ids()
            .rev()
            .max_by_key(|&i| self.statuses[i].rank());
        let named = worst.filter(|_| self.statuses.len() > 1);
        match (worst.map(|i| self.statuses[i]), self.backfill) {
            (Some(Status::Live) | None, _) if self.replying.is_some() => (Status::Fetching, None),
            (Some(Status::Live) | None, Some(backfill)) => (backfill, None),
            (Some(status), _) => (status, named.filter(|_| status != Status::Live)),
            (None, None) => (Status::Live, None),
        }
    }

    /// The first Account the server no longer accepts.
    pub fn signed_out(&self) -> Option<AccountId> {
        self.statuses.position(|&s| s == Status::SignedOut)
    }

    /// A failure is toasted once: the Cache stays in memory after it.
    pub fn notice_write_failure(&mut self, caches: &PerAccount<Cache>) {
        if caches.values().any(Cache::take_write_failure) {
            self.toast = Some(Toast::error("cache write failed"));
        }
    }

    /// When the Toast goes, the Status's countdown ticks or the Dwell ends,
    /// whichever is first; `None` while none will.
    pub fn changes_at(&self, now: Instant) -> Option<Instant> {
        let toast = self.toast.as_ref().map(|toast| toast.until);
        [toast, self.status().0.ticks_at(now), self.dwell.due()]
            .into_iter()
            .flatten()
            .min()
    }

    pub fn expire(&mut self, now: Instant) {
        if self.toast.as_ref().is_some_and(|toast| toast.until <= now) {
            self.toast = None;
        }
    }
}
