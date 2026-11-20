use tokio::time::Instant;

use crate::{
    awaited_reply::AwaitedReply, cache::Cache, draft_preview::DraftPreview, dwell::Dwell,
    folder_picker::FolderPicker, inbox_view::InboxView, link_hints::LinkHints,
    local_drafts::LocalDrafts, mode::Mode, reading::Reading, status::Status, thread::Key,
    toast::Toast, undo::Undo,
};

/// What the main screen shows over and beside the panes.
#[derive(Default)]
pub struct Screen {
    pub help: bool,
    pub picker: Option<FolderPicker>,
    pub toast: Option<Toast>,
    /// Each Account's sync connection's, by its place in this run.
    pub statuses: Vec<Status>,
    /// A page of older Messages under way on the fetch connection.
    pub backfill: Option<Status>,
    pub hints: Option<LinkHints>,
    pub mode: Mode,
    /// While `mode` is `Reading`.
    pub reading: Option<Reading>,
    pub dwell: Dwell,
    /// What an Undo may take back, the latest last.
    pub undo: Vec<Undo>,
    /// Over the whole screen, taking every key, while open.
    pub draft: Option<DraftPreview>,
    /// Under `draft`, taking every key it leaves.
    pub local: Option<LocalDrafts>,
    pub replying: Option<AwaitedReply>,
    /// The Messages in the Outbox not yet accepted.
    pub outbox: usize,
}

impl Screen {
    /// `rebuilt` when opening replaced the Cache, `failed` when the last
    /// sign-in did.
    pub fn opening(rebuilt: bool, statuses: Vec<Status>, failed: bool) -> Self {
        let toast = failed.then(|| Toast::error("sign-in failed"));
        Self {
            statuses,
            toast: toast.or_else(|| rebuilt.then(|| Toast::new("cache rebuilt"))),
            ..Self::default()
        }
    }

    pub fn list(&mut self) {
        self.mode = Mode::List;
        self.reading = None;
    }

    /// Back to the list once the selection left `thread`, the Thread Reading
    /// was opened on; true when it did.
    pub fn follow(&mut self, view: &InboxView, thread: Option<Key>) -> bool {
        let left = self.reading.is_some() && view.selected_key() != thread;
        if left {
            self.hints = None;
            self.list();
        }

        left
    }

    /// What the keys act on: a Draft's preview takes them all while open,
    /// then Local Drafts.
    pub const fn keys(&self) -> Mode {
        match (&self.draft, &self.local) {
            (Some(preview), _) if preview.queued => Mode::Queued,
            (Some(_), _) => Mode::Draft,
            (None, Some(_)) => Mode::LocalDrafts,
            (None, None) => self.mode,
        }
    }

    /// The worst sync connection's Status, with its Account when there are
    /// several; while all are live, a Reply's fetch, else the backfill's.
    pub fn status(&self) -> (Status, Option<usize>) {
        let worst = (0..self.statuses.len())
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
    pub fn signed_out(&self) -> Option<usize> {
        self.statuses.iter().position(|&s| s == Status::SignedOut)
    }

    /// A failure is toasted once: the Cache stays in memory after it.
    pub fn notice_write_failure(&mut self, caches: &[Cache]) {
        if caches.iter().any(Cache::take_write_failure) {
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
