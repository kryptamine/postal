use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::{
    compose::accounts::Accounts,
    compose::local_drafts::LocalDrafts,
    ui::drafts::draft_layer,
    ui::drafts::draft_preview::DraftPreview,
    ui::drafts::local_drafts_layer,
    ui::frame::theme::Theme,
    ui::input::keymap::{self, Binding, Hint},
    ui::popup::folder_picker::FolderPicker,
    ui::popup::hints_layer,
    ui::popup::layer_act::Act,
    ui::popup::picker_layer,
    ui::popup::settings::Settings,
    ui::popup::settings_layer,
    ui::preview::attachment_hints::AttachmentHints,
    ui::preview::link_hints::LinkHints,
    ui::state::mode::Mode,
};

/// One layer open over the panes. A new one is a variant here, a line of
/// `kinds!` and the module of its keys, its look and its help rows.
pub enum Layer {
    Settings(Settings),
    Draft(DraftPreview),
    LocalDrafts(LocalDrafts),
    Picker(FolderPicker),
    Hints(LinkHints),
    AttachmentHints(AttachmentHints),
}

impl Layer {
    pub fn key(&mut self, key: KeyEvent) -> Act {
        match self {
            Self::Settings(settings) => settings_layer::key(settings, key),
            Self::Draft(preview) => draft_layer::key(preview, key),
            Self::LocalDrafts(local) => local_drafts_layer::key(local, key),
            Self::Picker(picker) => picker_layer::key(picker, key),
            Self::Hints(hints) => hints_layer::key(hints, key),
            Self::AttachmentHints(hints) => hints_layer::attachment_key(hints, key),
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
            Self::Hints(_) | Self::AttachmentHints(_) => {}
        }
    }

    /// What the keys act on while this layer takes them; `None` leaves the
    /// panes' mode.
    pub const fn mode(&self) -> Option<Mode> {
        match self {
            Self::Settings(_) => Some(Mode::Settings),
            Self::Draft(preview) => Some(draft_layer::mode(preview)),
            Self::LocalDrafts(_) => Some(Mode::LocalDrafts),
            Self::Picker(_) | Self::Hints(_) | Self::AttachmentHints(_) => None,
        }
    }

    /// The hint line's keys while this layer takes them, in place of its
    /// mode's.
    pub const fn hints(&self) -> Option<&'static [Hint]> {
        match self {
            Self::Picker(_) => Some(picker_layer::HINTS),
            Self::Settings(settings) => settings_layer::hints(settings),
            Self::Draft(_) | Self::LocalDrafts(_) | Self::Hints(_) | Self::AttachmentHints(_) => {
                None
            }
        }
    }

    /// False for one laid out at the old width, which a resize closes.
    pub(super) const fn outlives_resize(&self) -> bool {
        !matches!(self, Self::Hints(_))
    }

    /// Where the layer sits: a higher one takes the keys and draws over.
    pub(super) const fn rank(&self) -> u8 {
        match self {
            Self::Hints(_) | Self::AttachmentHints(_) => 0,
            Self::Picker(_) => 1,
            Self::LocalDrafts(_) => 2,
            Self::Draft(_) => 3,
            Self::Settings(_) => 4,
        }
    }
}

/// The bindings listed in `mode` now, for the `?` popup and the hint line:
/// the panes' and the shared ones, then every layer's.
pub fn bindings(mode: Mode, signed_out: bool) -> impl Iterator<Item = &'static Binding> + Clone {
    keymap::pane_bindings()
        .chain(draft_layer::HELP)
        .chain(local_drafts_layer::HELP)
        .chain(settings_layer::HELP)
        .filter(move |b| b.modes.contains(&mode) && (signed_out || !b.signed_out))
}
