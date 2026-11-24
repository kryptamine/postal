use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::{
    cache::{Cache, Indexed},
    mail::body_fetch::open,
    mail::imap::MailServer,
    mail::snippet,
    mail::snippets,
    mail::sync::Error,
    mail::update::{Unindexed, Update},
};

/// Messages per batch: a Preview asked for meanwhile waits on one batch.
const BATCH: usize = 20;

/// The octets of a body indexed; what follows is mostly quoted history.
const OCTETS: u32 = 32 * 1024;

/// How often the screen is told how many bodies are left: counting them
/// reads the whole partial index, 12 ms at 500,000.
const COUNT_EVERY: Duration = Duration::from_secs(2);

/// How long the pass rests once Gmail says the day's download is spent.
const OVER_QUOTA: Duration = Duration::from_secs(24 * 60 * 60);

/// The pass over the bodies the index lacks, a batch a step.
pub struct BodyPass {
    more: bool,
    resting_until: Option<Instant>,
    last_count_at: Option<Instant>,
    last_told: Option<Unindexed>,
}

/// How one batch ended.
enum Batch {
    Indexed,
    NoneLeft,
    OverQuota,
}

impl BodyPass {
    pub const fn new() -> Self {
        Self {
            more: true,
            resting_until: None,
            last_count_at: None,
            last_told: None,
        }
    }

    pub fn due(&self) -> bool {
        self.more && !self.resting()
    }

    /// New rows were written: they may lack their bodies.
    pub const fn wake(&mut self) {
        self.more = true;
    }

    fn resting(&self) -> bool {
        self.resting_until
            .is_some_and(|until| Instant::now() < until)
    }

    /// One batch, telling the screen how many are left now and then, and
    /// once none is; `fetching` while the Folder pass brings more rows.
    pub async fn step(
        &mut self,
        server: &mut impl MailServer,
        cache: &Cache,
        fetching: bool,
        send: &impl Fn(Update),
    ) -> Result<(), Error> {
        if self
            .last_count_at
            .is_none_or(|at| at.elapsed() >= COUNT_EVERY)
        {
            self.tell(cache, fetching, send)?;
        }

        if self.resting() {
            return Ok(());
        }

        match batch(server, cache, send).await? {
            Batch::Indexed => {}
            Batch::NoneLeft => {
                self.more = false;
                self.tell(cache, fetching, send)?;
            }
            Batch::OverQuota => self.resting_until = Some(Instant::now() + OVER_QUOTA),
        }

        Ok(())
    }

    fn tell(&mut self, cache: &Cache, fetching: bool, send: &impl Fn(Update)) -> Result<(), Error> {
        self.last_count_at = Some(Instant::now());
        let now = Unindexed {
            bodies: cache.unindexed_count()?,
            fetching,
        };
        if self.last_told != Some(now) {
            self.last_told = Some(now);
            send(Update::Unindexed(now));
        }

        Ok(())
    }
}

/// Indexes the bodies of one batch of the newest Messages whose body the
/// index lacks, a Folder at a time, cutting their Snippets too when missing.
async fn batch(
    server: &mut impl MailServer,
    cache: &Cache,
    send: &impl Fn(Update),
) -> Result<Batch, Error> {
    let Some((folder, pending)) = cache.unindexed_bodies(BATCH)? else {
        return Ok(Batch::NoneLeft);
    };

    if !open(server, &folder).await? {
        return Ok(Batch::NoneLeft);
    }

    let parts: Vec<_> = pending
        .iter()
        .filter_map(|p| p.part.as_ref().map(|part| (p.uid, part)))
        .collect();
    // A batch refused for another reason is indexed without its bodies, or
    // the pass would ask for it again forever.
    let raw = match snippets::parts(server, &parts, OCTETS).await {
        Err(Error::Imap(error)) if error.refusal().is_some() => {
            tracing::warn!("indexing bodies of {folder} was refused: {error}");
            if error.to_string().to_ascii_lowercase().contains("overquota") {
                return Ok(Batch::OverQuota);
            }

            HashMap::new()
        }
        raw => raw?,
    };

    let indexed: Vec<_> = pending
        .iter()
        .map(|p| {
            let fetched = p.part.as_ref().zip(raw.get(&p.uid));
            Indexed {
                uid: p.uid,
                key: p.key,
                text: fetched.map(|(part, bytes)| snippet::text(bytes, part)),
                snippet: fetched
                    .filter(|_| p.snippet_missing)
                    .map(|(part, bytes)| snippet::from_part(bytes, part)),
            }
        })
        .collect();
    cache.index_bodies(&folder, &indexed)?;
    tracing::info!("indexed {} bodies of {folder}", indexed.len());
    send(Update::Inbox);
    Ok(Batch::Indexed)
}
