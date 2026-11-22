use std::{collections::HashSet, sync::Arc};

use chrono::{DateTime, FixedOffset, Local, Offset, Utc};

use crate::{
    cache::{self, Cache},
    domain::body_key::BodyKey,
    domain::folder::Folder,
    domain::folder_name::FolderName,
    domain::inbox::Inbox,
    domain::listing::Listing,
    domain::message::Message,
    domain::per_account::{AccountId, PerAccount},
    domain::shown::Shown,
    domain::thread::{Key, Thread},
    domain::thread_row::ThreadRow,
    domain::uid::Uid,
    render::rendered_previews::PreviewKey,
    render::view::View,
    ui::keymap::{By, Direction, Move, Scroll},
    ui::list::folder_list::FolderList,
    ui::list::matched::Matched,
    ui::list::search::Search,
};

/// The Message List's contents, selection and scroll position.
pub struct InboxView {
    /// The Folder shown; on every run All Inboxes, or the Inbox with one
    /// Account.
    pub folder: Shown,
    /// Each Account's address.
    pub accounts: PerAccount<Arc<str>>,
    /// Each Account's Folder and its oldest cached UID, when the server had
    /// nothing older there.
    pub nothing_below: PerAccount<Option<(FolderName, Uid)>>,
    /// The Account whose cache edge ends the rows, a Backfill of which
    /// shows more: the shown Folder's; in All Inboxes, as ADR 0014 picks;
    /// `None` once the server has nothing older.
    pub edge: Option<AccountId>,
    /// The shown Folder's rows; `None` until the Cache or the server has some.
    pub list: Option<FolderList>,
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
    /// The Messages shown with their Quote, by Account and body key.
    quoted: HashSet<(AccountId, BodyKey)>,
    /// The local offset as of the last `show`: chrono's `Local` rereads the
    /// time zone every second, and that allocates inside a frame.
    zone: FixedOffset,
    /// How many cards the list showed at most when last drawn.
    screen: usize,
    /// The Thread and Message selected when `take_rows` emptied the rows.
    kept: Option<(Option<Key>, Option<BodyKey>)>,
}

impl InboxView {
    /// `screen` is how many cards the first frame shows at most.
    pub fn loading(accounts: PerAccount<Arc<str>>, screen: usize) -> Self {
        Self {
            folder: match accounts.first() {
                Some(first) if accounts.len() == 1 => Shown::Folder(Folder::inbox(first)),
                _ => Shown::all_inboxes(),
            },
            nothing_below: accounts.map(|_| None),
            edge: None,
            accounts,
            list: None,
            search: None,
            selected: 0,
            offset: 0,
            preview_scroll: 0,
            html: View::default(),
            quoted: HashSet::new(),
            zone: Utc.fix(),
            screen,
            kept: None,
        }
    }

    /// Shows the Folder's Thread `rows`, newest first, and reads the cards
    /// around the selection; `since` is All Inboxes' edge. The selection
    /// stays on the same Thread, or on the same row when that Thread is gone.
    pub fn show(
        &mut self,
        rows: Vec<ThreadRow>,
        since: Option<i64>,
        caches: &PerAccount<Cache>,
    ) -> Result<(), cache::Error> {
        let (selected, body) = self.kept.take().unwrap_or((None, None));
        match &mut self.list {
            Some(list) => list.reset(rows, since),
            None => self.list = Some(FolderList::new(rows, since)),
        }

        if self.search.is_none() {
            self.keep_row(selected);
            self.fill(caches, self.screen)?;
            self.scroll_back((selected, body));
        }

        self.zone = Local::now().offset().fix();
        Ok(())
    }

    /// The rows' buffer, emptied, for the Cache to read the rows into anew;
    /// the selection is kept for `show`.
    pub fn take_rows(&mut self) -> Vec<ThreadRow> {
        self.kept = Some((self.selected_key(), self.selected_body()));
        self.list
            .as_mut()
            .map_or_else(Vec::new, FolderList::take_rows)
    }

    /// Reads the cards around the selection unless the window holds them,
    /// for a list `screen` cards tall. A Search shows its own.
    pub fn fill(&mut self, caches: &PerAccount<Cache>, screen: usize) -> Result<(), cache::Error> {
        self.screen = screen;
        match &mut self.list {
            Some(list) if self.search.is_none() => {
                list.fill(caches, &self.folder, self.selected, screen)
            }
            _ => Ok(()),
        }
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
    fn keep(&mut self, key: Option<Key>, body: Option<BodyKey>) {
        self.keep_row(key);
        self.scroll_back((key, body));
    }

    fn keep_row(&mut self, key: Option<Key>) {
        let last = self.rows().saturating_sub(1);
        self.selected = key
            .and_then(|key| self.position(key))
            .unwrap_or_else(|| self.selected.min(last));
    }

    /// The Preview back to its top unless the selection still shows the
    /// Thread and the Message `before` names; a Thread outside the window,
    /// its cards not read yet, is taken to show the same one.
    fn scroll_back(&mut self, before: (Option<Key>, Option<BodyKey>)) {
        let body = self.selected_body();
        if self.selected_key() != before.0 || body.is_some() && body != before.1 {
            self.preview_scroll = 0;
        }
    }

    /// Selects the Thread `key` when it is shown.
    pub fn select(&mut self, key: Key) {
        let before = (self.selected_key(), self.selected_body());
        if let Some(row) = self.position(key) {
            self.selected = row;
        }

        self.scroll_back(before);
    }

    /// Shows `folder` in place of the one shown, without its rows until
    /// `show`: any Search ends, and the newest Message will be selected.
    pub fn switch(&mut self, folder: Shown) {
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
        self.scroll_back((selected, body));
    }

    /// Whether the server has nothing older than the `cached` Messages
    /// `cache`, of `account`, holds of `folder`: as the last resync counted
    /// it, or a Backfill found.
    pub fn bottomed(
        &self,
        cache: &Cache,
        account: AccountId,
        (folder, cached): (&FolderName, usize),
    ) -> Result<bool, cache::Error> {
        if cache.whole(folder, cached)? {
            return Ok(true);
        }

        let Some(oldest) = cache.oldest_uid(folder)? else {
            return Ok(true);
        };

        Ok(self.nothing_below[account]
            .as_ref()
            .is_some_and(|(at, uid)| at == folder && *uid == oldest))
    }

    /// What tells the Preview of `message` at `width` from its others.
    pub fn preview_key(&self, message: &Message, width: u16) -> PreviewKey {
        PreviewKey {
            account: message.account,
            body: message.body_key,
            width,
            view: self.html,
            quoted: self.quoted.contains(&(message.account, message.body_key)),
        }
    }

    /// Shows the Quote of the Message `body` of `account`, or hides it.
    pub fn switch_quoted(&mut self, account: AccountId, body: BodyKey) {
        if !self.quoted.remove(&(account, body)) {
            self.quoted.insert((account, body));
        }
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
    pub fn selected_body(&self) -> Option<BodyKey> {
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
        match &self.search {
            Some(_) => self.thread_at(self.selected).map(|t| t.key),
            None => self.list.as_ref()?.key(self.selected),
        }
    }

    /// The cards shown: the Search's, else the shown Folder's window.
    fn listing(&self) -> Option<&Listing> {
        self.search.as_ref().map_or_else(
            || self.list.as_ref().map(FolderList::listing),
            |search| Some(search.listing()),
        )
    }

    /// The selected Thread's Messages, those found in other Folders too:
    /// the oldest, then the rest newest first.
    pub fn stack(&self) -> impl Iterator<Item = &Message> + Clone {
        let thread = self.thread_at(self.selected);
        self.listing()
            .into_iter()
            .flat_map(move |listing| listing.stack(thread))
    }

    /// How many rows are shown: all, or those a Search narrowed them to.
    pub fn rows(&self) -> usize {
        let all = || self.list.as_ref().map_or(0, FolderList::rows);
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
        Some((list.listing().messages(), list.thread(row)?, None))
    }

    /// The Thread shown at `row`.
    fn thread_at(&self, row: usize) -> Option<&Thread> {
        match &self.search {
            Some(search) => search.listing().threads().get(search.row(row)?),
            None => self.list.as_ref()?.thread(row),
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
