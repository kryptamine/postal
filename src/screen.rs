use tokio::time::Instant;

use crate::{
    awaited_reply::AwaitedReply, cache::Cache, draft_preview::DraftPreview, dwell::Dwell,
    folder_picker::FolderPicker, inbox_view::InboxView, link_hints::LinkHints, mode::Mode,
    reading::Reading, run::Work, status::Status, thread::Key, toast::Toast, undo::Undo,
};

/// What the main screen shows over and beside the panes.
#[derive(Default)]
pub struct Screen {
    pub help: bool,
    pub picker: Option<FolderPicker>,
    pub toast: Option<Toast>,
    /// The sync connection's.
    pub status: Status,
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
    pub replying: Option<AwaitedReply>,
    /// The Messages in the Outbox not yet accepted.
    pub outbox: usize,
}

impl Screen {
    pub fn opening<W>(cache: &Cache, work: &Work<W>) -> Self {
        let (status, toast) = match work {
            Work::Running(_) => (Status::Live, None),
            Work::SignedOut { failed } => (
                Status::SignedOut,
                failed.then(|| Toast::error("sign-in failed")),
            ),
        };

        Self {
            status,
            toast: toast.or_else(|| cache.rebuilt().then(|| Toast::new("cache rebuilt"))),
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

    /// What the keys act on: a Draft's preview takes them all while open.
    pub const fn keys(&self) -> Mode {
        if self.draft.is_some() {
            Mode::Draft
        } else {
            self.mode
        }
    }

    /// The sync connection's Status, or while that one is live, a Reply's
    /// fetch, else the backfill's.
    pub const fn status(&self) -> Status {
        match (self.status, self.backfill) {
            (Status::Live, _) if self.replying.is_some() => Status::Fetching,
            (Status::Live, Some(backfill)) => backfill,
            (status, _) => status,
        }
    }

    /// A failure is toasted once: the Cache stays in memory after it.
    pub fn notice_write_failure(&mut self, cache: &Cache) {
        if cache.take_write_failure() {
            self.toast = Some(Toast::error("cache write failed"));
        }
    }

    /// When the Toast goes, the Status's countdown ticks or the Dwell ends,
    /// whichever is first; `None` while none will.
    pub fn changes_at(&self, now: Instant) -> Option<Instant> {
        let toast = self.toast.as_ref().map(|toast| toast.until);
        [toast, self.status().ticks_at(now), self.dwell.due()]
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
