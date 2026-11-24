use std::{cmp::Ordering, sync::Arc};

use nucleo_matcher::{
    Config, Matcher, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::{
    domain::counts::Counts,
    domain::folder::{Folder, Role},
    domain::per_account::{AccountId, PerAccount},
    domain::shown::Shown,
};

/// What the Folder picked is for.
#[derive(Clone, Copy)]
pub enum Purpose {
    Show,
    /// The selected Messages of this Account move to it.
    MoveTo(AccountId),
}

/// The Folder Picker: every Folder, narrowed by a typed Query with the
/// Search matcher, and the row selected. With several Accounts, All Inboxes
/// comes first, each Account has a section headed by its address, and Local
/// Drafts comes after them all.
pub struct FolderPicker {
    purpose: Purpose,
    folders: Vec<Shown>,
    /// The section each Folder is listed in: All Inboxes', each Account's,
    /// then Local Drafts'; one section with one Account.
    sections: Vec<usize>,
    /// Each Account's address, when there are several.
    headings: Vec<Arc<str>>,
    /// The label and full path of each Folder, as the Query matches them.
    haystacks: Vec<Utf32String>,
    query: String,
    /// Indices into `folders`, by section, then best match first; all of
    /// them, in order, for an empty Query.
    hits: Vec<usize>,
    /// Index into `hits`.
    selected: usize,
    matcher: Matcher,
}

/// One row of the Folder Picker.
pub enum Row<'a> {
    /// The address heading an Account's section.
    Heading(&'a str),
    Folder(&'a Shown),
}

impl FolderPicker {
    /// Lists `folders`, with `shown` among them and selected; `accounts`
    /// are the addresses of this run.
    pub fn new(folders: Vec<Folder>, shown: &Shown, accounts: &PerAccount<Arc<str>>) -> Self {
        let headings = if accounts.len() > 1 {
            accounts.values().cloned().collect()
        } else {
            Vec::new()
        };

        Self::empty(Purpose::Show, headings).filled(folders, shown)
    }

    /// Lists the Folders of `account` but `shown`, to move Messages to.
    pub fn moving(folders: Vec<Folder>, shown: &Shown, account: AccountId) -> Self {
        Self::empty(Purpose::MoveTo(account), Vec::new()).filled(folders, shown)
    }

    fn empty(purpose: Purpose, headings: Vec<Arc<str>>) -> Self {
        Self {
            purpose,
            folders: Vec::new(),
            sections: Vec::new(),
            headings,
            haystacks: Vec::new(),
            query: String::new(),
            hits: Vec::new(),
            selected: 0,
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    fn filled(mut self, folders: Vec<Folder>, shown: &Shown) -> Self {
        self.reload(folders, shown);
        self
    }

    pub const fn purpose(&self) -> Purpose {
        self.purpose
    }

    /// Lists `folders` anew, with All Inboxes, Local Drafts, the Query and
    /// the selected Folder kept; `shown` is listed even before the Cache
    /// knows it.
    pub fn reload(&mut self, folders: Vec<Folder>, shown: &Shown) {
        let selected = self.chosen().unwrap_or(shown).clone();
        if let Purpose::MoveTo(account) = self.purpose {
            let mut folders: Vec<_> = folders
                .into_iter()
                .filter(|f| f.account == account && !shown.is(f.account, &f.name))
                .map(Shown::Folder)
                .collect();
            folders.sort_by(order);
            self.sections = vec![0; folders.len()];
            self.list(folders, &selected);
            return;
        }

        let all_inboxes = (!self.headings.is_empty()).then(|| counted_all_inboxes(&folders));
        let mut folders: Vec<_> = folders.into_iter().map(Shown::Folder).collect();
        if matches!(shown, Shown::Folder(_)) && !folders.iter().any(|f| f.same(shown)) {
            folders.push(shown.clone());
        }

        folders.extend(all_inboxes);
        folders.push(Shown::LocalDrafts);
        let section = |shown: &Shown| match shown {
            _ if self.headings.is_empty() => 0,
            Shown::AllInboxes { .. } => 0,
            Shown::LocalDrafts => self.headings.len() + 1,
            Shown::Folder(folder) => folder.account.place() + 1,
        };

        folders.sort_by(|a, b| section(a).cmp(&section(b)).then_with(|| order(a, b)));
        self.sections = folders.iter().map(section).collect();
        self.list(folders, &selected);
    }

    /// Lists `folders`, sorted into their sections, with `selected` kept.
    fn list(&mut self, folders: Vec<Shown>, selected: &Shown) {
        self.haystacks = folders.iter().map(haystack).collect();
        self.folders = folders;
        self.rematch();
        self.selected = self
            .hits
            .iter()
            .position(|&i| self.folders[i].same(selected))
            .unwrap_or(0);
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.rematch();
        self.selected = 0;
    }

    pub fn pop(&mut self) {
        if self.query.pop().is_some() {
            self.rematch();
            self.selected = 0;
        }
    }

    pub fn down(&mut self) {
        self.selected = (self.selected + 1).min(self.hits.len().saturating_sub(1));
    }

    pub const fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// The selected Folder; `None` while the Query matches none.
    pub fn chosen(&self) -> Option<&Shown> {
        self.folders.get(*self.hits.get(self.selected)?)
    }

    /// The selected row, counting the headings above it.
    pub fn selected(&self) -> usize {
        let headings = self
            .heading_at()
            .take(self.selected + 1)
            .filter(|&heading| heading)
            .count();
        self.selected + headings
    }

    pub const fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// The rows shown, each section headed when there are several.
    pub fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        self.hits
            .iter()
            .zip(self.heading_at())
            .flat_map(|(&i, heading)| {
                let heading = heading.then(|| self.heading(self.sections[i])).flatten();
                heading
                    .map(|address| Row::Heading(address))
                    .into_iter()
                    .chain([Row::Folder(&self.folders[i])])
            })
    }

    /// The address heading `section`, when it is an Account's.
    fn heading(&self, section: usize) -> Option<&Arc<str>> {
        self.headings.get(section.checked_sub(1)?)
    }

    /// For each hit, whether it opens an Account's section.
    fn heading_at(&self) -> impl Iterator<Item = bool> + '_ {
        self.hits.iter().enumerate().map(move |(at, &i)| {
            let section = self.sections[i];
            let opens = at == 0 || self.sections[self.hits[at - 1]] != section;
            opens && self.heading(section).is_some()
        })
    }

    fn rematch(&mut self) {
        let pattern = Pattern::new(
            &self.query,
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut scored: Vec<_> = self
            .haystacks
            .iter()
            .enumerate()
            .filter_map(|(i, haystack)| {
                Some((pattern.score(haystack.slice(..), &mut self.matcher)?, i))
            })
            .collect();
        // A tie keeps the listed order.
        let sections = &self.sections;
        scored.sort_by(|a, b| {
            sections[a.1]
                .cmp(&sections[b.1])
                .then(b.0.cmp(&a.0))
                .then(a.1.cmp(&b.1))
        });
        self.hits = scored.into_iter().map(|(_, i)| i).collect();
    }
}

/// All Inboxes, its counts the sum of the Inboxes' counted.
fn counted_all_inboxes(folders: &[Folder]) -> Shown {
    let inboxes = folders.iter().filter(|f| f.role == Some(Role::Inbox));
    Shown::AllInboxes {
        counts: inboxes.filter_map(|f| f.counts).reduce(Counts::plus),
    }
}

/// All Inboxes first, then the server's Folders in their own order, Local
/// Drafts right after the Drafts role.
fn order(a: &Shown, b: &Shown) -> Ordering {
    let rank = |shown: &Shown| match shown {
        Shown::AllInboxes { .. } => (false, None, false),
        Shown::Folder(folder) => (folder.role.is_none(), folder.role, false),
        Shown::LocalDrafts => (false, Some(Role::Drafts), true),
    };

    match (a, b) {
        (Shown::Folder(a), Shown::Folder(b)) => a.order(b),
        _ => rank(a).cmp(&rank(b)),
    }
}

/// A special-use Folder answers to its label as well as to its path.
fn haystack(shown: &Shown) -> Utf32String {
    match shown.folder() {
        Some(folder) if folder.role.is_none() => Utf32String::from(&*folder.path),
        Some(folder) => Utf32String::from(format!("{}\n{}", folder.label(), folder.path)),
        None => Utf32String::from(shown.label()),
    }
}
