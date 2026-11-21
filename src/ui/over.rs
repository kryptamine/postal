use crate::{
    compose::local_drafts::LocalDrafts, ui::draft_preview::DraftPreview,
    ui::folder_picker::FolderPicker, ui::link_hints::LinkHints, ui::settings::Settings,
};

/// What is open over the panes; several may be at once, one over another.
#[derive(Default)]
pub struct Over {
    pub settings: Option<Settings>,
    /// Over Local Drafts when opened from them.
    pub draft: Option<DraftPreview>,
    pub local: Option<LocalDrafts>,
    pub picker: Option<FolderPicker>,
    pub hints: Option<LinkHints>,
}

/// One open layer, borrowed from `Over`.
pub enum Layer<'a> {
    Settings(&'a Settings),
    Draft(&'a DraftPreview),
    LocalDrafts(&'a LocalDrafts),
    Picker(&'a FolderPicker),
    Hints,
}

impl Over {
    /// The open layers, the one taking the keys first; each is drawn over
    /// those after it.
    pub fn layers(&self) -> impl DoubleEndedIterator<Item = Layer<'_>> {
        [
            self.settings.as_ref().map(Layer::Settings),
            self.draft.as_ref().map(Layer::Draft),
            self.local.as_ref().map(Layer::LocalDrafts),
            self.picker.as_ref().map(Layer::Picker),
            self.hints.as_ref().map(|_| Layer::Hints),
        ]
        .into_iter()
        .flatten()
    }

    /// The layer taking the keys; `None` leaves them to the panes.
    pub fn top(&self) -> Option<Layer<'_>> {
        self.layers().next()
    }
}
