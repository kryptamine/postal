use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Local, Offset, Utc};

use crate::{
    folder::Folder,
    inbox::Inbox,
    keymap::{By, Direction, Move, Scroll},
    listing::Listing,
    matched::Matched,
    message::Message,
    preview_pane::LABELS,
    search::Search,
    thread::{Key, Thread},
    view::View,
};

/// The Message List's contents, selection and scroll position.
pub struct InboxView {
    /// The Folder shown; on every run All Inboxes, or the Inbox with one
    /// Account.
    pub folder: Folder,
    /// Each Account's address, by its place in this run.
    pub accounts: Vec<Arc<str>>,
    /// Each Account's Folder and its oldest cached UID, when the server had
    /// nothing older there.
    pub nothing_below: Vec<Option<(Arc<str>, u32)>>,
    /// The Account whose cache edge ends the rows, a Backfill of which
    /// shows more: the shown Folder's; in All Inboxes, as ADR 0014 picks.
    pub edge: Option<usize>,
    /// The shown Folder's rows; `None` until the Cache or the server has some.
    pub list: Option<Listing>,
    /// Shows its own rows, from every Folder; `selected` and `offset` index those.
    pub search: Option<Search>,
    pub selected: usize,
    /// Index of the first visible card; kept by the widget so the selection stays on screen.
    pub offset: usize,
    /// The Preview's first row shown; back to 0 whenever the selection
    /// moves to another Message, and kept in range by the Preview widget.
    pub preview_scroll: usize,
    /// How the Preview shows HTML mail, for every Message.
    pub html: View,
    /// The local offset as of the last `show`: chrono's `Local` rereads the
    /// time zone every second, and that allocates inside a frame.
    zone: FixedOffset,
}

impl InboxView {
    pub fn loading(accounts: Vec<Arc<str>>) -> Self {
        Self {
            folder: if accounts.len() > 1 {
                Folder::all_inboxes()
            } else {
                Folder::inbox(0)
            },
            nothing_below: vec![None; accounts.len()],
            edge: None,
            accounts,
            list: None,
            search: None,
            selected: 0,
            offset: 0,
            preview_scroll: 0,
            html: View::default(),
            zone: Utc.fix(),
        }
    }

    /// Shows the Folder's Messages, already ordered newest first. The
    /// selection stays on the same Thread, or on the same row when that
    /// Thread is gone.
    pub fn show(&mut self, inbox: Inbox) {
        let (selected, body) = (self.selected_key(), self.selected_body());
        self.list = Some(Listing::new(inbox));
        if self.search.is_none() {
            self.keep(selected, body);
        }

        self.zone = Local::now().offset().fix();
    }

    /// Matches the Search under way, if any, over `everywhere` anew.
    pub fn research(&mut self, everywhere: Inbox) {
        let (selected, body) = (self.selected_key(), self.selected_body());
        if let Some(search) = &mut self.search {
            search.reload(Listing::new(everywhere));
            self.keep(selected, body);
        }
    }

    /// Selects the Thread `key`, else the same row clamped to the rows shown.
    fn keep(&mut self, key: Option<Key>, body: Option<i64>) {
        let last = self.rows().saturating_sub(1);
        self.selected = key
            .and_then(|key| self.position(key))
            .unwrap_or_else(|| self.selected.min(last));
        if self.selected_body() != body {
            self.preview_scroll = 0;
        }
    }

    /// Selects the Thread `key` when it is shown.
    pub fn select(&mut self, key: Key) {
        let body = self.selected_body();
        if let Some(row) = self.position(key) {
            self.selected = row;
        }

        if self.selected_body() != body {
            self.preview_scroll = 0;
        }
    }

    /// Shows `folder` in place of the one shown, without its rows until
    /// `show`: any Search ends, and the newest Message will be selected.
    pub fn switch(&mut self, folder: Folder) {
        self.folder = folder;
        self.list = None;
        self.search = None;
        self.selected = 0;
        self.offset = 0;
        self.preview_scroll = 0;
    }

    /// Starts a Search over what `everywhere` reads, or resumes the one
    /// under way; false before any rows. Either way, Esc while typing comes
    /// back to the selection now.
    pub fn start_search<E>(
        &mut self,
        everywhere: impl FnOnce() -> Result<Inbox, E>,
    ) -> Result<bool, E> {
        if self.list.is_none() {
            return Ok(false);
        }

        let (before, body) = (self.selected_key(), self.selected_body());
        if self.search.is_none() {
            self.search = Some(Search::new(Listing::new(everywhere()?)));
            self.offset = 0;
            self.keep(before, body);
        }

        if let Some(search) = &mut self.search {
            search.before = before;
        }

        Ok(true)
    }

    /// Edits the Query and selects the best match.
    pub fn edit_query(&mut self, edit: impl FnOnce(&mut Search)) {
        let selected = self.selected_body();
        let Some(search) = &mut self.search else {
            return;
        };

        edit(search);
        self.selected = 0;
        if self.selected_body() != selected {
            self.preview_scroll = 0;
        }
    }

    /// Back to the shown Folder's list, on the Thread selected before the
    /// last `/` when `restore` or when the Folder lacks the one selected now,
    /// else on that one.
    pub fn end_search(&mut self, restore: bool) {
        let (selected, body) = (self.selected_key(), self.selected_body());
        let Some(search) = self.search.take() else {
            return;
        };

        let found = selected
            .filter(|_| !restore)
            .and_then(|key| self.position(key));
        let before = || search.before.and_then(|key| self.position(key));
        self.selected = found.or_else(before).unwrap_or(0);
        self.offset = 0;
        if self.selected_body() != body {
            self.preview_scroll = 0;
        }
    }

    /// Whether the server had nothing older than what `account` caches of
    /// `folder`, whose oldest cached UID is `oldest`.
    pub fn bottomed(&self, account: usize, folder: &str, oldest: u32) -> bool {
        self.nothing_below
            .get(account)
            .and_then(Option::as_ref)
            .is_some_and(|(at, uid)| **at == *folder && *uid == oldest)
    }

    /// The address of `account`, which a header shows only with several.
    pub fn account_label(&self, account: usize) -> Option<&str> {
        let several = self.accounts.len() > 1;
        self.accounts.get(account).filter(|_| several).map(|a| &**a)
    }

    /// The header rows of a Preview, with the row under them: a rule in
    /// the Preview, blank in a stack.
    pub fn header_rows(&self) -> usize {
        let account = usize::from(self.accounts.len() > 1);
        LABELS.len() - 1 + account + 1
    }

    pub fn now(&self) -> DateTime<FixedOffset> {
        Utc::now().with_timezone(&self.zone)
    }

    pub fn apply(&mut self, movement: Move) {
        let last = self.rows().saturating_sub(1);
        let selected = match movement {
            Move::Down => (self.selected + 1).min(last),
            Move::Up => self.selected.saturating_sub(1),
            Move::First => 0,
            Move::Last => last,
        };

        if selected != self.selected {
            self.selected = selected;
            self.preview_scroll = 0;
        }
    }

    /// Past the last page is fine: the Preview widget clamps it.
    pub const fn scroll(&mut self, scroll: Scroll, half_page: usize) {
        let rows = match scroll.by {
            By::Line => 1,
            By::HalfPage => half_page,
            By::End => usize::MAX,
        };

        self.preview_scroll = match scroll.direction {
            Direction::Down => self.preview_scroll.saturating_add(rows),
            Direction::Up => self.preview_scroll.saturating_sub(rows),
        };
    }

    /// The body key of the selected Thread's newest Message: a UID names
    /// another Message in each Folder.
    pub fn selected_body(&self) -> Option<i64> {
        self.selected_message().map(|m| m.body_key)
    }

    /// The newest Message of the selected Thread.
    pub fn selected_message(&self) -> Option<&Message> {
        self.row(self.selected)
    }

    /// Whether any of the selected Thread's Messages in its Folder is Unread.
    pub fn selected_unread(&self) -> bool {
        self.thread_at(self.selected).is_some_and(|t| t.unread)
    }

    pub fn selected_key(&self) -> Option<Key> {
        self.thread_at(self.selected).map(|t| t.key)
    }

    /// The rows shown: the Search's, else the shown Folder's.
    pub fn listing(&self) -> Option<&Listing> {
        self.search
            .as_ref()
            .map_or(self.list.as_ref(), |search| Some(search.listing()))
    }

    /// The selected Thread's Messages, those found in other Folders too,
    /// oldest first.
    pub fn stack(&self) -> impl DoubleEndedIterator<Item = &Message> + Clone {
        let thread = self.thread_at(self.selected);
        self.listing()
            .into_iter()
            .flat_map(move |listing| listing.stack(thread))
    }

    /// How many rows are shown: all, or those a Search narrowed them to.
    pub fn rows(&self) -> usize {
        let all = || self.list.as_ref().map_or(0, |list| list.threads.len());
        self.search.as_ref().map_or_else(all, Search::rows)
    }

    /// The newest Message of the Thread shown at `row`.
    pub fn row(&self, row: usize) -> Option<&Message> {
        self.listing()?.newest(self.thread_at(row)?)
    }

    /// The Messages and Thread of the card at `row`, with its matched units
    /// while searching.
    pub fn card(&mut self, row: usize) -> Option<(&[Message], &Thread, Option<Matched<'_>>)> {
        if let Some(search) = &mut self.search {
            let (messages, thread, matched) = search.card(row)?;
            return Some((messages, thread, Some(matched)));
        }

        let list = self.list.as_ref()?;
        Some((&list.messages, list.threads.get(row)?, None))
    }

    /// The Thread shown at `row`.
    fn thread_at(&self, row: usize) -> Option<&Thread> {
        match &self.search {
            Some(search) => search.listing().threads.get(search.row(row)?),
            None => self.list.as_ref()?.threads.get(row),
        }
    }

    /// The row that shows the Thread `key`.
    fn position(&self, key: Key) -> Option<usize> {
        match &self.search {
            Some(search) => search.position(search.listing().find(key)?),
            None => self.list.as_ref()?.find(key),
        }
    }
}
