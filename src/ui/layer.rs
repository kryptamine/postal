use std::iter;

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    compose::accounts::Accounts, compose::local_drafts::LocalDrafts, ui::draft_layer,
    ui::draft_preview::DraftPreview, ui::folder_picker::FolderPicker, ui::hints_layer,
    ui::keymap::Binding, ui::layer_act::Act, ui::link_hints::LinkHints, ui::local_drafts_layer,
    ui::mode::Mode, ui::picker_layer, ui::settings::Settings, ui::settings_layer, ui::theme::Theme,
};

/// One layer open over the panes. A new one is a variant here, a line of
/// `kinds!` and the module of its keys, its look and its help rows.
pub enum Layer {
    Settings(Settings),
    Draft(DraftPreview),
    LocalDrafts(LocalDrafts),
    Picker(FolderPicker),
    Hints(LinkHints),
}

impl Layer {
    pub fn key(&mut self, key: KeyEvent) -> Act {
        match self {
            Self::Settings(settings) => settings_layer::key(settings, key),
            Self::Draft(preview) => draft_layer::key(preview, key),
            Self::LocalDrafts(local) => local_drafts_layer::key(local, key),
            Self::Picker(picker) => picker_layer::key(picker, key),
            Self::Hints(hints) => hints_layer::key(hints, key),
        }
    }

    pub fn draw(&self, frame: &mut Frame, theme: &Theme, accounts: Accounts, body: Rect) {
        match self {
            Self::Settings(settings) => settings_layer::draw(settings, frame, theme, body),
            Self::Draft(preview) => draft_layer::draw(preview, frame, theme, accounts, body),
            Self::LocalDrafts(local) => {
                local_drafts_layer::draw(local, frame, theme, accounts, body);
            }
            Self::Picker(picker) => picker_layer::draw(picker, frame, theme, body),
            // The panes draw their own labels.
            Self::Hints(_) => {}
        }
    }

    /// What the keys act on while this layer takes them; `None` leaves the
    /// panes' mode.
    pub const fn mode(&self) -> Option<Mode> {
        match self {
            Self::Settings(_) => Some(Mode::Settings),
            Self::Draft(preview) => Some(draft_layer::mode(preview)),
            Self::LocalDrafts(_) => Some(Mode::LocalDrafts),
            Self::Picker(_) | Self::Hints(_) => None,
        }
    }

    /// False for one laid out at the old width, which a resize closes.
    pub(super) const fn outlives_resize(&self) -> bool {
        !matches!(self, Self::Hints(_))
    }

    /// Where the layer sits: a higher one takes the keys and draws over.
    pub(super) const fn rank(&self) -> u8 {
        match self {
            Self::Hints(_) => 0,
            Self::Picker(_) => 1,
            Self::LocalDrafts(_) => 2,
            Self::Draft(_) => 3,
            Self::Settings(_) => 4,
        }
    }
}

/// Every layer's rows of the `?` popup and the hint line.
pub fn help() -> impl Iterator<Item = &'static Binding> + Clone {
    iter::empty()
        .chain(draft_layer::HELP)
        .chain(local_drafts_layer::HELP)
        .chain(settings_layer::HELP)
}
