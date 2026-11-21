use std::mem;

use crate::{ui::layer::Layer, ui::layer_kind::Kind};

/// What is open over the panes, the top last: at most one layer of each
/// kind, stacked by `Layer::rank` whatever order they opened in.
#[derive(Default)]
pub struct Over(Vec<Layer>);

impl Over {
    /// Opens `layer` in its place, in place of one of its kind.
    pub fn open(&mut self, layer: impl Into<Layer>) {
        let layer = layer.into();
        self.0
            .retain(|open| mem::discriminant(open) != mem::discriminant(&layer));
        let at = self.0.partition_point(|open| open.rank() < layer.rank());
        self.0.insert(at, layer);
    }

    /// The open layers, the bottom first.
    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.0.iter()
    }

    /// The layer taking the keys; `None` leaves them to the panes.
    pub fn top(&self) -> Option<&Layer> {
        self.0.last()
    }

    pub fn top_mut(&mut self) -> Option<&mut Layer> {
        self.0.last_mut()
    }

    pub fn close_top(&mut self) {
        self.0.pop();
    }

    pub fn resized(&mut self) {
        self.0.retain(Layer::outlives_resize);
    }

    pub fn get<K: Kind>(&self) -> Option<&K> {
        self.0.iter().find_map(K::of)
    }

    pub fn get_mut<K: Kind>(&mut self) -> Option<&mut K> {
        self.0.iter_mut().find_map(K::of_mut)
    }

    pub fn is_open<K: Kind>(&self) -> bool {
        self.get::<K>().is_some()
    }

    pub fn take<K: Kind>(&mut self) -> Option<K> {
        let at = self.0.iter().position(|layer| K::of(layer).is_some())?;
        K::into_kind(self.0.remove(at))
    }

    pub fn close<K: Kind>(&mut self) {
        self.take::<K>();
    }
}
