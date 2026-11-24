use std::time::Duration;

use crate::{
    cache::Cache, domain::folder_name::FolderName, domain::uid::Uid, mail::body_fetch::open,
    mail::body_index::BodyPass, mail::envelope_pass, mail::folder_pass::FolderPass,
    mail::imap::MailServer, mail::sent_pass, mail::snippets, mail::sync::Error,
    mail::update::Update,
};

/// How long the fetch connection goes without a request before the pass
/// over Sent and each step over envelopes, so the first Preview never
/// waits on one.
const QUIET: Duration = Duration::from_secs(1);

/// How often an indexed Cache is looked at again for bodies new mail brought.
const RECHECK: Duration = Duration::from_secs(5);

/// What the fetch connection does while no request waits, a step at a
/// time, so a request that comes meanwhile goes next.
pub struct SpareWork {
    /// Rows of this Folder below this UID still wait for their Snippets.
    snippets_below: Option<(FolderName, Uid)>,
    /// The pass over Sent, once per connection.
    sent_due: bool,
    /// The three long passes, after Sent's, take turns a step each: every
    /// Folder whole into the Cache, the bodies the index lacks, and every
    /// envelope for the Correspondents.
    folders: FolderPass,
    bodies: BodyPass,
    envelopes_due: bool,
    turn: Turn,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Turn {
    Folders,
    Bodies,
    Envelopes,
}

impl Turn {
    const fn after(self) -> Self {
        match self {
            Self::Folders => Self::Bodies,
            Self::Bodies => Self::Envelopes,
            Self::Envelopes => Self::Folders,
        }
    }
}

impl SpareWork {
    pub const fn new() -> Self {
        Self {
            snippets_below: None,
            sent_due: true,
            folders: FolderPass::new(),
            bodies: BodyPass::new(),
            envelopes_due: true,
            turn: Turn::Folders,
        }
    }

    /// How long to wait for a request before the next step.
    pub fn patience(&self) -> Duration {
        if self.snippets_below.is_some() {
            Duration::ZERO
        } else if self.sent_due {
            QUIET
        } else if self.folders.due() || self.bodies.due() {
            Duration::ZERO
        } else if self.envelopes_due {
            QUIET
        } else {
            RECHECK
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

    /// One batch of Snippets, else the pass over Sent, else a step of the
    /// long pass whose turn it is.
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
        } else {
            self.long_step(server, cache, send).await?;
        }

        Ok(())
    }

    /// A step of the first due long pass from `turn` on; with none due, a
    /// recheck for bodies new mail brought.
    async fn long_step(
        &mut self,
        server: &mut impl MailServer,
        cache: &Cache,
        send: &impl Fn(Update),
    ) -> Result<(), Error> {
        let due = |turn| match turn {
            Turn::Folders => self.folders.due(),
            Turn::Bodies => self.bodies.due(),
            Turn::Envelopes => self.envelopes_due,
        };

        let (first, second) = (self.turn, self.turn.after());
        let turn = [first, second, second.after()]
            .into_iter()
            .find(|&turn| due(turn))
            .unwrap_or(Turn::Bodies);
        self.turn = turn.after();
        match turn {
            Turn::Folders => {
                self.folders.step(server, cache, send).await?;
                self.bodies.wake();
            }
            Turn::Bodies => {
                let fetching = self.folders.due();
                self.bodies.step(server, cache, fetching, send).await?;
            }
            Turn::Envelopes => {
                self.envelopes_due = envelope_pass::step(server, cache, send).await?;
            }
        }

        Ok(())
    }
}
