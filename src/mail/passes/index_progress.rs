use std::time::{Duration, Instant};

use crate::{
    cache::Cache, domain::folder_name::FolderName, domain::indexing::Indexing, mail::sync::Error,
    mail::update::Update,
};

/// How often the screen is told how far the index is while passes run:
/// counting takes 22 ms at 500,000.
const EVERY: Duration = Duration::from_secs(5);

/// Tells the screen how far the search index is, when it changed.
pub struct IndexProgress {
    counted_at: Option<Instant>,
    told: Option<Indexing>,
}

impl IndexProgress {
    pub const fn new() -> Self {
        Self {
            counted_at: None,
            told: None,
        }
    }

    /// At most every `EVERY`; `walked` are the Folders fetched whole.
    pub fn tell(
        &mut self,
        cache: &Cache,
        walked: &[FolderName],
        send: &impl Fn(Update),
    ) -> Result<(), Error> {
        if self.counted_at.is_some_and(|at| at.elapsed() < EVERY) {
            return Ok(());
        }

        self.tell_now(cache, walked, send)
    }

    /// As `tell`, at once: a pass just ended, and the screen should not
    /// show what was left a moment ago.
    pub fn tell_now(
        &mut self,
        cache: &Cache,
        walked: &[FolderName],
        send: &impl Fn(Update),
    ) -> Result<(), Error> {
        self.counted_at = Some(Instant::now());
        let now = cache.indexing(walked)?;
        if self.told != Some(now) {
            self.told = Some(now);
            send(Update::Indexing(now));
        }

        Ok(())
    }
}
