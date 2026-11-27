use tokio::time::Instant;

use crate::{
    cache::Cache,
    compose::outbox::Outbox,
    domain::indexing::Indexing,
    domain::per_account::{AccountId, PerAccount},
    domain::thread::Key,
    mail::status::Status,
    ui::drafts::awaiting::Waiting,
    ui::list::inbox_view::InboxView,
    ui::outside::contacts_check::ContactsCheck,
    ui::popup::help::Help,
    ui::popup::layer::Layer,
    ui::popup::over::Over,
    ui::preview::link_hints::LinkHints,
    ui::preview::reading::Reading,
    ui::state::dwell::Dwell,
    ui::state::mode::Mode,
    ui::state::showing::Showing,
    ui::state::toast::Toast,
    ui::state::undo::Undo,
};

/// What the main screen shows over and beside the panes.
#[derive(Default)]
pub struct Screen {
    /// The `?` popup: over everything, `over` included, taking every key
    /// while open.
    help: Option<Help>,
    toast: Option<Toast>,
    /// Each Account's sync connection's.
    pub statuses: PerAccount<Status>,
    /// A page of older Messages under way on the fetch connection.
    backfill: Option<Status>,
    /// How far each Account's search index is from every Message.
    pub indexing: PerAccount<Indexing>,
    showing: Showing,
    pub over: Over,
    pub dwell: Dwell,
    /// What an Undo may take back, the latest last.
    undo: Vec<Undo>,
    /// What a Reply's or a Forward's Draft waits on before it opens.
    awaited: Option<Waiting>,
    /// The Messages in the Outbox not yet accepted.
    outbox: usize,
    /// The read of Contacts started by turning them on, while under way.
    pub contacts_check: ContactsCheck,
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
            indexing: statuses.map(|_| Indexing::default()),
            statuses,
            toast: toast.or_else(|| rebuilt.then(|| Toast::new("cache rebuilt"))),
            ..Self::default()
        }
    }

    /// How far the search index of every Account is from every Message.
    pub fn index_progress(&self) -> Indexing {
        let each = self.indexing.values().copied();
        each.fold(Indexing::default(), Indexing::plus)
    }

    pub fn list(&mut self) {
        self.showing = Showing::List;
    }

    pub const fn showing(&self) -> &Showing {
        &self.showing
    }

    pub fn show(&mut self, showing: Showing) {
        self.showing = showing;
    }

    pub const fn reading_mut(&mut self) -> Option<&mut Reading> {
        self.showing.reading_mut()
    }

    /// Reading beside the layers over it, borrowed apart so one is drawn
    /// while the other is read.
    pub const fn reading_under(&mut self) -> (Option<&mut Reading>, &Over) {
        (self.showing.reading_mut(), &self.over)
    }

    pub const fn toast(&self) -> Option<&Toast> {
        self.toast.as_ref()
    }

    /// `None` takes down the Toast shown.
    pub fn set_toast(&mut self, toast: impl Into<Option<Toast>>) {
        self.toast = toast.into();
    }

    pub const fn help_mut(&mut self) -> Option<&mut Help> {
        self.help.as_mut()
    }

    pub const fn helping(&self) -> bool {
        self.help.is_some()
    }

    pub fn open_help(&mut self) {
        self.help = Some(Help::default());
    }

    pub fn close_help(&mut self) {
        self.help = None;
    }

    /// The Messages waiting in `outbox`, counted again after it changed.
    pub fn recount(&mut self, outbox: &Outbox) {
        self.outbox = outbox.count();
    }

    pub const fn outbox(&self) -> usize {
        self.outbox
    }

    /// A page of older Messages under way, `None` once it is in or failed.
    pub const fn set_backfill(&mut self, backfill: Option<Status>) {
        self.backfill = backfill;
    }

    pub fn push_undo(&mut self, undo: Undo) {
        self.undo.push(undo);
    }

    /// The latest change an Undo may take back.
    pub fn pop_undo(&mut self) -> Option<Undo> {
        self.undo.pop()
    }

    /// Drops the Undo of each Send that left the Outbox: `u` never unsends
    /// what SMTP has, and moves on to the entry before.
    pub fn forget_sent(&mut self) {
        self.undo
            .retain(|undo| !matches!(undo, Undo::Send(path) if !path.exists()));
    }

    pub const fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn await_parts(&mut self, waiting: Waiting) {
        self.awaited = Some(waiting);
    }

    pub const fn awaited_mut(&mut self) -> Option<&mut Waiting> {
        self.awaited.as_mut()
    }

    pub const fn take_awaited(&mut self) -> Option<Waiting> {
        self.awaited.take()
    }

    /// Opens `layer`, else shows the Toast saying why it cannot open.
    pub fn open_or_toast(&mut self, layer: Result<impl Into<Layer>, Toast>) {
        match layer {
            Ok(layer) => self.over.open(layer),
            Err(toast) => self.toast = Some(toast),
        }
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
    /// several; while all are live, a Reply's or a Forward's fetch, else
    /// the backfill's.
    pub fn status(&self) -> (Status, Option<AccountId>) {
        let worst = self
            .statuses
            .ids()
            .rev()
            .max_by_key(|&i| self.statuses[i].rank());
        let named = worst.filter(|_| self.statuses.len() > 1);
        match (worst.map(|i| self.statuses[i]), self.backfill) {
            (Some(Status::Live) | None, _) if self.fetching() => (Status::Fetching, None),
            (Some(Status::Live) | None, Some(backfill)) => (backfill, None),
            (Some(status), _) => (status, named.filter(|_| status != Status::Live)),
            (None, None) => (Status::Live, None),
        }
    }

    const fn fetching(&self) -> bool {
        self.awaited.is_some()
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

    /// When the Toast goes or counts down, the Status's countdown ticks or
    /// the Dwell ends, whichever is first; `None` while none will.
    pub fn changes_at(&self, now: Instant) -> Option<Instant> {
        let toast = self.toast.as_ref().map(|toast| toast.until);
        let countdown = self.toast.as_ref().and_then(|toast| toast.ticks_at(now));
        let contacts = self.contacts_check.due(now);
        [
            toast,
            countdown,
            self.status().0.ticks_at(now),
            self.dwell.due(),
            contacts,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub fn expire(&mut self, now: Instant) {
        if self.toast.as_ref().is_some_and(|toast| toast.until <= now) {
            self.toast = None;
        }

        if let Some(toast) = self.contacts_check.finished() {
            self.toast = Some(toast);
        }
    }
}
