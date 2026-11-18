use nucleo_matcher::{
    Config, Matcher, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::folder::Folder;

/// The Folder Picker: every Folder, narrowed by a typed Query with the
/// Search matcher, and the row selected.
pub struct FolderPicker {
    folders: Vec<Folder>,
    /// The label and full path of each Folder, as the Query matches them.
    haystacks: Vec<Utf32String>,
    query: String,
    /// Indices into `folders`, best match first; all of them, in order, for
    /// an empty Query.
    hits: Vec<usize>,
    /// Index into `hits`.
    selected: usize,
    matcher: Matcher,
}

impl FolderPicker {
    /// Lists `folders`, with `shown` among them and selected.
    pub fn new(folders: Vec<Folder>, shown: &Folder) -> Self {
        let mut picker = Self {
            folders: Vec::new(),
            haystacks: Vec::new(),
            query: String::new(),
            hits: Vec::new(),
            selected: 0,
            matcher: Matcher::new(Config::DEFAULT),
        };

        picker.reload(folders, shown);
        picker
    }

    /// Lists `folders` anew, the Query and the selected Folder kept; `shown`
    /// is listed even before the Cache knows it.
    pub fn reload(&mut self, mut folders: Vec<Folder>, shown: &Folder) {
        let selected = self
            .chosen()
            .map_or_else(|| shown.name.clone(), |f| f.name.clone());
        if !folders.iter().any(|f| f.name == shown.name) {
            folders.push(shown.clone());
            folders.sort_by(Folder::order);
        }

        self.haystacks = folders.iter().map(haystack).collect();
        self.folders = folders;
        self.rematch();
        self.selected = self
            .hits
            .iter()
            .position(|&i| self.folders[i].name == selected)
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

    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// The Folders shown, best match first.
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &Folder> {
        self.hits.iter().map(|&i| &self.folders[i])
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
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.hits = scored.into_iter().map(|(_, i)| i).collect();
    }
}

/// A special-use Folder answers to its label as well as to its path.
fn haystack(folder: &Folder) -> Utf32String {
    match folder.role {
        Some(_) => Utf32String::from(format!("{}\n{}", folder.label(), folder.path)),
        None => Utf32String::from(&*folder.path),
    }
}
