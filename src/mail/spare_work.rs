use std::time::Duration;

use crate::{
    cache::Cache, domain::folder_name::FolderName, domain::uid::Uid, mail::body_fetch::open,
    mail::body_index::BodyPass, mail::envelope_pass, mail::folder_pass::FolderPass,
    mail::imap::MailServer, mail::index_progress::IndexProgress, mail::sent_pass, mail::snippets,
    mail::sync::Error, mail::update::Update,
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
    progress: IndexProgress,
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

/// The step `SpareWork` takes next.
enum Work {
    Snippets,
    Sent,
    Long(Turn),
    /// No pass is due: a look for bodies new mail brought.
    Recheck,
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
            progress: IndexProgress::new(),
        }
    }

    /// How long to wait for a request before the next step.
    pub fn patience(&self) -> Duration {
        match self.next() {
            Work::Snippets | Work::Long(Turn::Folders | Turn::Bodies) => Duration::ZERO,
            Work::Sent | Work::Long(Turn::Envelopes) => QUIET,
            Work::Recheck => RECHECK,
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

    /// Snippets a Backfill left, else the pass over Sent, else the first due
    /// long pass from `turn` on.
    fn next(&self) -> Work {
        if self.snippets_below.is_some() {
            return Work::Snippets;
        }

        if self.sent_due {
            return Work::Sent;
        }

        let due = |turn| match turn {
            Turn::Folders => self.folders.due(),
            Turn::Bodies => self.bodies.due(),
            Turn::Envelopes => self.envelopes_due,
        };

        let turns = [self.turn, self.turn.after(), self.turn.after().after()];
        turns
            .into_iter()
            .find(|&turn| due(turn))
            .map_or(Work::Recheck, Work::Long)
    }

    pub async fn step(
        &mut self,
        server: &mut impl MailServer,
        cache: &Cache,
        send: &impl Fn(Update),
    ) -> Result<(), Error> {
        match self.next() {
            Work::Snippets => {
                let Some((folder, below)) = &self.snippets_below else {
                    return Ok(());
                };

                let written = || send(Update::Inbox);
                if !open(server, folder).await?
                    || !snippets::batch(server, cache, folder, Some(*below), written).await?
                {
                    self.snippets_below = None;
                }

                return Ok(());
            }
            Work::Sent => {
                self.sent_due = false;
                return sent_pass::read(server, cache, send).await;
            }
            Work::Long(turn) => {
                self.turn = turn.after();
                match turn {
                    Turn::Folders => {
                        self.folders.step(server, cache, send).await?;
                        self.bodies.wake();
                    }
                    Turn::Bodies => self.bodies.step(server, cache, send).await?,
                    Turn::Envelopes => {
                        self.envelopes_due = envelope_pass::step(server, cache, send).await?;
                    }
                }
            }
            Work::Recheck => {
                self.bodies.wake();
                self.bodies.step(server, cache, send).await?;
            }
        }

        let walked = self.folders.walked();
        if self.folders.due() || self.bodies.due() {
            self.progress.tell(cache, walked, send)
        } else {
            self.progress.tell_now(cache, walked, send)
        }
    }
}
