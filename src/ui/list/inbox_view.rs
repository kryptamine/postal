use std::{collections::HashSet, sync::Arc};

use chrono::{DateTime, FixedOffset, Local, Offset, Utc};

use crate::{
    account::config::ContactsChoice,
    cache::{self, Cache},
    domain::body_key::BodyKey,
    domain::folder::{Folder, Role},
    domain::folder_name::FolderName,
    domain::listing::Listing,
    domain::message::Message,
    domain::per_account::{AccountId, PerAccount},
    domain::shown::Shown,
    domain::thread::{Key, Thread},
    domain::thread_row::ThreadRow,
    domain::uid::Uid,
    render::rendered_previews::PreviewKey,
    render::view::View,
    ui::input::keymap::{By, Direction, Move, Scroll},
    ui::list::folder_list::FolderList,
    ui::list::matched::Matched,
    ui::list::picks::Picks,
    ui::list::search::Search,
};

/// The Message List's contents, selection and scroll position.
pub struct InboxView {
    /// The Folder shown; on every run All Inboxes, or the Inbox with one
    /// Account.
    folder: Shown,
    /// Each Account's address.
    accounts: PerAccount<Arc<str>>,
    /// Each Account's Folder and its oldest cached UID, when the server had
    /// nothing older there.
    nothing_below: PerAccount<Option<(FolderName, Uid)>>,
    /// The Account whose cache edge ends the rows, a Backfill of which
    /// shows more: the shown Folder's; in All Inboxes, as ADR 0014 picks;
    /// `None` once the server has nothing older.
    edge: Option<AccountId>,
    /// The shown Folder's rows; `None` until the Cache or the server has some.
    list: Option<FolderList>,
    /// Shows its own rows, from every Folder; `selected` and `offset` index those.
    search: Option<Search>,
    selected: usize,
    /// Index of the first visible card; kept by the widget so the selection stays on screen.
    offset: usize,
    /// The Preview's first row shown; back to 0 whenever the selection
    /// moves to another Message, and kept in range by the Preview widget.
    preview_scroll: usize,
    /// How the Preview shows HTML mail, for every Message.
    html: View,
    picks: Picks,
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
            picks: Picks::default(),
            quoted: HashSet::new(),
            zone: Utc.fix(),
            screen,
            kept: None,
        }
    }

    pub const fn folder(&self) -> &Shown {
        &self.folder
    }

    pub const fn accounts(&self) -> &PerAccount<Arc<str>> {
        &self.accounts
    }

    /// The Search under way, which shows its own rows.
    pub const fn search(&self) -> Option<&Search> {
        self.search.as_ref()
    }

    pub const fn searching(&self) -> bool {
        self.search.is_some()
    }

    /// Whether the shown Folder has its rows yet, from the Cache or the server.
    pub const fn loaded(&self) -> bool {
        self.list.is_some()
    }

    /// The Role of the Folder whose rows are shown; `None` in a Search,
    /// whose rows come from every Folder.
    pub fn shown_role(&self) -> Option<Role> {
        self.search.is_none().then(|| self.folder.role()).flatten()
    }

    /// The row selected, of the Search's rows while one is under way.
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// Scrolls the list so the selected card shows whole, `highest` telling
    /// the highest first row that keeps it so.
    pub fn scroll_list(&mut self, highest: impl FnOnce(&Self) -> usize) {
        self.offset = if self.selected < self.offset {
            self.selected
        } else {
            self.offset.max(highest(self))
        };
    }

    /// The first row the list shows.
    pub const fn list_top(&self) -> usize {
        self.offset
    }

    pub const fn preview_scroll(&self) -> usize {
        self.preview_scroll
    }

    /// The Preview's first row shown, no lower than `last_top`.
    pub fn clamp_preview_scroll(&mut self, last_top: usize) -> usize {
        self.preview_scroll = self.preview_scroll.min(last_top);
        self.preview_scroll
    }

    /// Shows HTML mail in the other View, from the Preview's top.
    pub const fn switch_view(&mut self) {
        self.html = self.html.other();
        self.preview_scroll = 0;
    }

    /// The Account and Folder to ask a page of older Messages of, once the
    /// selection is within `near_end` rows of the end; `None` in a Search,
    /// which matches only the Cache, or once the server has nothing older.
    pub fn older_page(&self, near_end: usize) -> Option<(AccountId, &FolderName)> {
        if self.list.is_none() || self.search.is_some() || self.selected + near_end < self.rows() {
            return None;
        }

        // With no edge, every Account has shown all it holds (ADR 0014).
        let account = self.edge?;
        Some((account, self.folder.of(account)?))
    }

    /// A page of older Messages of `folder` of `account` came in; the server
    /// had nothing below `oldest` when it is given.
    pub fn paged(&mut self, account: AccountId, folder: FolderName, oldest: Option<Uid>) {
        self.nothing_below[account] = oldest.map(|uid| (folder, uid));
    }

    /// Suggests for the last word of the Search's Query, if one is under way.
    pub fn suggest(&mut self, caches: &PerAccount<Cache>) -> Result<(), cache::Error> {
        self.search
            .as_mut()
            .map_or_else(|| Ok(()), |search| search.suggest(caches))
    }

    pub const fn picking(&self) -> bool {
        !self.picks.is_empty()
    }

    pub const fn unpick(&mut self) {
        self.picks.clear();
    }

    /// Shows the Folder's Thread `rows`, newest first, and reads the cards
    /// around the selection; `since` and `edge` end the rows (ADR 0014). The
    /// selection stays on the same Thread, or on the same row when it is gone.
    pub fn show(
        &mut self,
        rows: Vec<ThreadRow>,
        (since, edge): (Option<i64>, Option<AccountId>),
        caches: &PerAccount<Cache>,
    ) -> Result<(), cache::Error> {
        self.edge = edge;
        let (selected, body) = self.kept.take().unwrap_or((None, None));
        match &mut self.list {
            Some(list) => list.reset(rows, since),
            None => self.list = Some(FolderList::new(rows, since)),
        }

        if self.search.is_none() {
            self.keep_row(selected);
            self.rebase();
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
    /// the Search's while one is under way, for a list `screen` cards tall.
    pub fn fill(&mut self, caches: &PerAccount<Cache>, screen: usize) -> Result<(), cache::Error> {
        self.screen = screen;
        if let Some(search) = &mut self.search {
            return search.fill(caches, self.selected, screen);
        }

        match &mut self.list {
            Some(list) => list.fill(caches, &self.folder, self.selected, screen),
            None => Ok(()),
        }
    }

    /// Runs the Search under way, if any, anew over what the Cache holds now.
    pub fn research(&mut self, caches: &PerAccount<Cache>) -> Result<(), cache::Error> {
        let (selected, body) = (self.selected_key(), self.selected_body());
        if let Some(search) = &mut self.search {
            search.run(caches, &self.folder)?;
            self.keep_row(selected);
            self.rebase();
            self.fill(caches, self.screen)?;
            self.scroll_back((selected, body));
        }

        Ok(())
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
        self.picks.clear();
        self.list = None;
        self.search = None;
        self.selected = 0;
        self.offset = 0;
        self.preview_scroll = 0;
    }

    /// Starts a Search over every cached Folder, or resumes the one under
    /// way; false before any rows. Either way, Esc while typing comes back
    /// to the selection now.
    pub fn start_search(
        &mut self,
        caches: &PerAccount<Cache>,
        contacts: ContactsChoice,
    ) -> Result<bool, cache::Error> {
        if self.list.is_none() {
            return Ok(false);
        }

        let (before, body) = (self.selected_key(), self.selected_body());
        if self.search.is_none() {
            self.picks.clear();
            self.search = Some(Search::new(caches, &self.folder, &self.accounts, contacts)?);
            self.offset = 0;
            self.keep_row(before);
            self.fill(caches, self.screen)?;
            self.scroll_back((before, body));
        }

        if let Some(search) = &mut self.search {
            search.before = before;
        }

        Ok(true)
    }

    /// Edits the Query, `edit` saying whether it changed, then runs it
    /// anew and selects the best match.
    pub fn edit_query(
        &mut self,
        caches: &PerAccount<Cache>,
        edit: impl FnOnce(&mut Search) -> bool,
    ) -> Result<(), cache::Error> {
        let selected = self.selected_body();
        let Some(search) = &mut self.search else {
            return Ok(());
        };

        if !edit(search) {
            return Ok(());
        }

        search.run(caches, &self.folder)?;
        self.selected = 0;
        self.rebase();
        self.fill(caches, self.screen)?;
        if self.selected_body() != selected {
            self.preview_scroll = 0;
        }

        Ok(())
    }

    /// Closes the Search's menu; whether it was open.
    pub fn close_menu(&mut self) -> bool {
        self.search.as_mut().is_some_and(Search::close_menu)
    }

    /// Back to the shown Folder's list, on the Thread selected before the
    /// last `/` when `restore` or when the Folder lacks the one selected now,
    /// else on that one.
    pub fn end_search(&mut self, restore: bool) {
        let (selected, body) = (self.selected_key(), self.selected_body());
        let Some(search) = self.search.take() else {
            return;
        };

        self.picks.clear();
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

    /// The Message at `cursor` in the selected Thread's stack, as Reading
    /// has it; without a cursor, the selected one.
    pub fn at_cursor(&self, cursor: Option<BodyKey>) -> Option<&Message> {
        cursor.map_or_else(
            || self.selected_message(),
            |cursor| self.stack().find(|m| m.body_key == cursor),
        )
    }

    /// Whether any of the selected Thread's Messages in its Folder is Unread.
    pub fn selected_unread(&self) -> bool {
        self.thread_at(self.selected).is_some_and(|t| t.unread)
    }

    pub fn selected_key(&self) -> Option<Key> {
        match &self.search {
            Some(search) => search.key(self.selected),
            None => self.list.as_ref()?.key(self.selected),
        }
    }

    /// The cards shown: the Search's window, else the shown Folder's.
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
            Some(search) => search.thread_at(row),
            None => self.list.as_ref()?.thread(row),
        }
    }

    /// The row that shows the Thread `key`.
    fn position(&self, key: Key) -> Option<usize> {
        position(self.search.as_ref(), self.list.as_ref(), key)
    }

    fn key(&self, row: usize) -> Option<Key> {
        match &self.search {
            Some(search) => search.key(row),
            None => self.list.as_ref()?.key(row),
        }
    }

    /// The Picks started again on the rows shown now.
    fn rebase(&mut self) {
        let (search, list) = (self.search.as_ref(), self.list.as_ref());
        self.picks.rebase(|key| position(search, list, key));
    }

    /// Picks from the selected Thread on, or from where the Picks start,
    /// to the Thread `movement` selects.
    pub fn stretch(&mut self, movement: Move) {
        if let Some(key) = self.selected_key() {
            self.picks.start(key, self.selected);
            self.apply(movement);
        }
    }

    fn picked_keys(&self) -> impl Iterator<Item = Key> + '_ {
        let rows = self.picks.range(self.selected).into_iter().flatten();
        rows.filter_map(|row| self.key(row))
    }

    /// Whether the Thread at `row` is picked.
    pub fn picked(&self, row: usize) -> bool {
        let range = self.picks.range(self.selected);
        range.is_some_and(|range| range.contains(&row))
    }

    pub fn count_picked(&self) -> usize {
        self.picks.range(self.selected).map_or(0, Iterator::count)
    }

    /// The picked Threads shown now, read whole from the Cache: those
    /// outside the window too. `None` with no Picks.
    pub fn picked_listing(
        &mut self,
        caches: &PerAccount<Cache>,
    ) -> Result<Option<Listing>, cache::Error> {
        if self.picks.is_empty() {
            return Ok(None);
        }

        let keys = self.picked_keys().collect();
        let listing = match (&mut self.search, &self.list) {
            (Some(search), _) => search.listing_of(caches, &keys)?,
            (None, Some(list)) => list.listing_of(caches, &self.folder, &keys)?,
            (None, None) => Listing::empty(),
        };

        Ok(Some(listing))
    }

    /// The Accounts of the picked Threads.
    pub fn picked_accounts(&self) -> HashSet<AccountId> {
        self.picked_keys().map(|key| *key.account()).collect()
    }
}

fn position(search: Option<&Search>, list: Option<&FolderList>, key: Key) -> Option<usize> {
    match search {
        Some(search) => search.find(key),
        None => list?.find(key),
    }
}
