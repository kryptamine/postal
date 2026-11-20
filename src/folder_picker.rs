use std::sync::Arc;

use nucleo_matcher::{
    Config, Matcher, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::folder::{Folder, Role};

/// The Folder Picker: every Folder, narrowed by a typed Query with the
/// Search matcher, and the row selected. With several Accounts, each has a
/// section headed by its address, and Local Drafts comes after them all.
pub struct FolderPicker {
    folders: Vec<Folder>,
    /// The section each Folder is listed in: its Account's, then Local
    /// Drafts' after them all; one section with one Account.
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
    Folder(&'a Folder),
}

impl FolderPicker {
    /// Lists `folders`, with `shown` among them and selected; `accounts`
    /// are the addresses of this run.
    pub fn new(folders: Vec<Folder>, shown: &Folder, accounts: &[Arc<str>]) -> Self {
        let headings = if accounts.len() > 1 {
            accounts.to_vec()
        } else {
            Vec::new()
        };

        let mut picker = Self {
            folders: Vec::new(),
            sections: Vec::new(),
            headings,
            haystacks: Vec::new(),
            query: String::new(),
            hits: Vec::new(),
            selected: 0,
            matcher: Matcher::new(Config::DEFAULT),
        };

        picker.reload(folders, shown);
        picker
    }

    /// Lists `folders` anew, with Local Drafts, the Query and the selected
    /// Folder kept; `shown` is listed even before the Cache knows it.
    pub fn reload(&mut self, mut folders: Vec<Folder>, shown: &Folder) {
        let selected = self.chosen().unwrap_or(shown).clone();
        if !folders.iter().any(|f| same(f, shown)) {
            folders.push(shown.clone());
        }

        folders.push(Folder::local_drafts());
        let section = |folder: &Folder| match folder.role {
            _ if self.headings.is_empty() => 0,
            Some(Role::LocalDrafts) => self.headings.len(),
            _ => folder.account,
        };

        folders.sort_by(|a, b| section(a).cmp(&section(b)).then_with(|| a.order(b)));
        self.sections = folders.iter().map(section).collect();
        self.haystacks = folders.iter().map(haystack).collect();
        self.folders = folders;
        self.rematch();
        self.selected = self
            .hits
            .iter()
            .position(|&i| same(&self.folders[i], &selected))
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
    pub fn chosen(&self) -> Option<&Folder> {
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
                let heading = heading
                    .then(|| self.headings.get(self.sections[i]))
                    .flatten();
                heading
                    .map(|address| Row::Heading(address))
                    .into_iter()
                    .chain([Row::Folder(&self.folders[i])])
            })
    }

    /// For each hit, whether it opens an Account's section.
    fn heading_at(&self) -> impl Iterator<Item = bool> + '_ {
        let headed = !self.headings.is_empty();
        self.hits.iter().enumerate().map(move |(at, &i)| {
            let section = self.sections[i];
            let opens = at == 0 || self.sections[self.hits[at - 1]] != section;
            headed && opens && section < self.headings.len()
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

/// Folders are known by Account and name; Local Drafts by its role.
fn same(a: &Folder, b: &Folder) -> bool {
    let local = |f: &Folder| f.role == Some(Role::LocalDrafts);
    local(a) == local(b) && (local(a) || (a.account == b.account && a.name == b.name))
}

/// A special-use Folder answers to its label as well as to its path.
fn haystack(folder: &Folder) -> Utf32String {
    match folder.role {
        Some(_) => Utf32String::from(format!("{}\n{}", folder.label(), folder.path)),
        None => Utf32String::from(&*folder.path),
    }
}
