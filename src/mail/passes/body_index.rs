use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use crate::{
    cache::{Cache, Indexed},
    mail::fetch::examine::open,
    mail::imap::ImapSession,
    mail::passes::snippet,
    mail::passes::snippets,
    mail::sync::Error,
    mail::update::Update,
};

/// Messages per batch: a Preview asked for meanwhile waits on one batch.
const BATCH: usize = 20;

/// The octets of a body indexed; what follows is mostly quoted history.
const OCTETS: u32 = 32 * 1024;

/// How long the pass rests once Gmail says the day's download is spent.
const OVER_QUOTA: Duration = Duration::from_secs(24 * 60 * 60);

/// The pass over the bodies the index lacks, a batch a step.
pub struct BodyPass {
    more: bool,
    resting_until: Option<Instant>,
}

/// How one batch ended.
enum Batch {
    /// This many bodies.
    Indexed(usize),
    NoneLeft,
    OverQuota,
}

impl BodyPass {
    pub const fn new() -> Self {
        Self {
            more: true,
            resting_until: None,
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

    pub async fn step(
        &mut self,
        server: &mut ImapSession,
        cache: &Cache,
        send: &impl Fn(Update),
    ) -> Result<(), Error> {
        if self.resting() {
            return Ok(());
        }

        match batch(server, cache, BATCH, send).await? {
            Batch::Indexed(_) => {}
            Batch::NoneLeft => self.more = false,
            Batch::OverQuota => self.resting_until = Some(Instant::now() + OVER_QUOTA),
        }

        Ok(())
    }
}

/// Indexes the bodies of the `count` newest Messages the index lacks, or of
/// all of them when fewer lack it.
pub async fn index_newest(
    server: &mut ImapSession,
    cache: &Cache,
    count: usize,
) -> Result<(), Error> {
    let mut left = count;
    while left > 0 {
        match batch(server, cache, left.min(BATCH), &|_| {}).await? {
            Batch::Indexed(indexed) => left = left.saturating_sub(indexed),
            Batch::NoneLeft | Batch::OverQuota => return Ok(()),
        }
    }

    Ok(())
}

/// Indexes the bodies of up to `limit` of the newest Messages whose body
/// the index lacks, a Folder at a time, cutting their Snippets too when missing.
async fn batch(
    server: &mut ImapSession,
    cache: &Cache,
    limit: usize,
    send: &impl Fn(Update),
) -> Result<Batch, Error> {
    let Some((folder, pending)) = cache.unindexed_bodies(limit)? else {
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
            if error.over_quota() {
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
    Ok(Batch::Indexed(indexed.len()))
}
