use std::time::Duration;

use crate::{
    cache::Cache, domain::folder_name::FolderName, domain::uid::Uid, mail::body_fetch::open,
    mail::envelope_pass, mail::imap::MailServer, mail::sent_pass, mail::snippets,
    mail::sync::Error, mail::update::Update,
};

/// How long the fetch connection goes without a request before each pass
/// over envelopes, so the first Preview never waits on one.
const QUIET: Duration = Duration::from_secs(1);

/// What the fetch connection does while no request waits, a step at a
/// time, so a request that comes meanwhile goes next.
pub struct SpareWork {
    /// Rows of this Folder below this UID still wait for their Snippets.
    snippets_below: Option<(FolderName, Uid)>,
    /// The pass over Sent, once per connection.
    sent_due: bool,
    /// The pass over every envelope, a batch a step, after Sent's.
    envelopes_due: bool,
}

impl SpareWork {
    pub const fn new() -> Self {
        Self {
            snippets_below: None,
            sent_due: true,
            envelopes_due: true,
        }
    }

    /// How long to wait for a request before the next step; `None` when
    /// there is none to take.
    pub const fn patience(&self) -> Option<Duration> {
        if self.snippets_below.is_some() {
            Some(Duration::ZERO)
        } else if self.sent_due || self.envelopes_due {
            Some(QUIET)
        } else {
            None
        }
    }

    /// A Backfill of `folder` left its rows below `below` without Snippets.
    pub fn backfilled(&mut self, folder: FolderName, below: Option<Uid>) {
        // The higher bound keeps an earlier page's uncut rows in.
        self.snippets_below = match (self.snippets_below.take(), below) {
            (Some((same, earlier)), Some(below)) if same == folder => {
                Some((same, earlier.max(below)))
            }
            (kept, None) => kept,
            (_, Some(below)) => Some((folder, below)),
        };
    }

    /// One batch of Snippets, else the pass over Sent, else one batch of
    /// envelopes.
    pub async fn step(
        &mut self,
        server: &mut impl MailServer,
        cache: &Cache,
        send: &impl Fn(Update),
    ) -> Result<(), Error> {
        if let Some((folder, below)) = &self.snippets_below {
            let written = || send(Update::Inbox);
            if !open(server, folder).await?
                || !snippets::batch(server, cache, folder, Some(*below), written).await?
            {
                self.snippets_below = None;
            }
        } else if self.sent_due {
            self.sent_due = false;
            sent_pass::read(server, cache, send).await?;
        } else if self.envelopes_due {
            self.envelopes_due = envelope_pass::step(server, cache, send).await?;
        }

        Ok(())
    }
}
