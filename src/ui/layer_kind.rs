use crate::{
    compose::local_drafts::LocalDrafts, ui::draft_preview::DraftPreview,
    ui::folder_picker::FolderPicker, ui::layer::Layer, ui::link_hints::LinkHints,
    ui::settings::Settings,
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
