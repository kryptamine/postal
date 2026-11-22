use crate::{
    compose::local_drafts::LocalDrafts, ui::drafts::draft_preview::DraftPreview,
    ui::popup::folder_picker::FolderPicker, ui::popup::layer::Layer, ui::popup::settings::Settings,
    ui::preview::link_hints::LinkHints,
};

/// A type one variant of `Layer` holds.
pub trait Kind: Into<Layer> {
    fn of(layer: &Layer) -> Option<&Self>;
    fn of_mut(layer: &mut Layer) -> Option<&mut Self>;
    fn into_kind(layer: Layer) -> Option<Self>;
}

macro_rules! kinds {
    ($($variant:ident($kind:ty)),* $(,)?) => {$(
        impl From<$kind> for Layer {
            fn from(kind: $kind) -> Self {
                Self::$variant(kind)
            }
        }

        impl Kind for $kind {
            fn of(layer: &Layer) -> Option<&Self> {
                match layer {
                    Layer::$variant(kind) => Some(kind),
                    _ => None,
                }
            }

            fn of_mut(layer: &mut Layer) -> Option<&mut Self> {
                match layer {
                    Layer::$variant(kind) => Some(kind),
                    _ => None,
                }
            }

            fn into_kind(layer: Layer) -> Option<Self> {
                match layer {
                    Layer::$variant(kind) => Some(kind),
                    _ => None,
                }
            }
        }
    )*};
}

kinds!(
    Settings(Settings),
    Draft(DraftPreview),
    LocalDrafts(LocalDrafts),
    Picker(FolderPicker),
    Hints(LinkHints),
);
