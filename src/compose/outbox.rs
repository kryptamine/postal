use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use chrono::{DateTime, TimeDelta, Utc};

use crate::{
    account::atomic_file,
    compose::draft_file,
    compose::draft_notes::DraftNotes,
    compose::notes::{self, Notes},
};

/// Only the user may read their own writing.
const MODE: u32 = 0o600;

/// The Messages sent and not yet accepted, in `outbox/`: each as it goes
/// out in `<id>.eml`, `<id>.sending` once SMTP has it, `<id>.sent` once
/// SMTP accepts it and until its copy is in Sent, its notes in `<id>.json`
/// (ADR 0013).
pub struct Outbox {
    /// `None` without a data directory: nothing can be queued then.
    data: Option<PathBuf>,
    /// How long a Send is held for `u` to take it back.
    delay: Duration,
}

/// One Message in the Outbox.
pub struct Queued {
    /// Its file.
    pub path: PathBuf,
    /// Accepted by SMTP; only its copy in Sent is left.
    pub sent: bool,
    /// As they read when it was listed; `None` without any, or when they
    /// do not read.
    pub notes: Option<Notes>,
}

/// A Send taken back: the Draft it was again.
pub struct TakenBack {
    pub path: PathBuf,
    pub text: String,
    pub notes: DraftNotes,
}

impl Outbox {
    /// The Outbox under `data`, beside `drafts/`, holding each Send for
    /// `delay`.
    pub const fn new(data: Option<PathBuf>, delay: Duration) -> Self {
        Self { data, delay }
    }

    pub const fn delay(&self) -> Duration {
        self.delay
    }

    fn dir(&self, name: &str) -> io::Result<PathBuf> {
        let data = self
            .data
            .as_ref()
            .ok_or_else(|| io::Error::other("no data directory"))?;
        Ok(data.join(name))
    }

    /// The Messages not yet accepted.
    pub fn count(&self) -> usize {
        self.queued()
            .map_or(0, |queued| queued.iter().filter(|q| !q.sent).count())
    }

    /// Every Message in the Outbox, oldest first.
    pub fn queued(&self) -> io::Result<Vec<Queued>> {
        let Some(data) = &self.data else {
            return Ok(Vec::new());
        };

        let entries = match std::fs::read_dir(data.join("outbox")) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };

        let mut queued = Vec::new();
        for entry in entries {
            let path = entry?.path();
            let sent = match path.extension().and_then(OsStr::to_str) {
                Some("eml" | "sending") => false,
                Some("sent") => true,
                _ => continue,
            };

            let notes = Notes::read(&path).unwrap_or_else(|error| {
                tracing::warn!("reading the notes of {} failed: {error}", path.display());
                None
            });

            queued.push(Queued { path, sent, notes });
        }

        // A file's name starts with the time its Draft was made.
        queued.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(queued)
    }

    /// The earliest time a Message is held until that is still to come.
    pub fn next_hold(&self) -> Option<DateTime<Utc>> {
        let (queued, now) = (self.queued().ok()?, Utc::now());
        let unsent = queued.iter().filter(|queued| !queued.sent);
        let holds = unsent.filter_map(Queued::hold_until);
        holds.filter(|&until| until > now).min()
    }

    /// Moves the Draft at `draft` here as `message`, its notes beside it,
    /// held for the delay; the path it is queued at.
    pub fn queue(&self, draft: &Path, message: &[u8], notes: Notes) -> io::Result<PathBuf> {
        let path = self
            .dir("outbox")?
            .join(draft.file_name().unwrap_or_default());
        let delay = TimeDelta::from_std(self.delay)
            .ok()
            .filter(|d| !d.is_zero());
        let held = delay.and_then(|delay| Utc::now().checked_add_signed(delay));
        let notes = Notes {
            hold_until: held,
            ..notes
        };

        // The notes first: a Message is in the Outbox once its file is.
        notes.write(&path, MODE)?;
        atomic_file::write(&path, message, MODE)?;
        // Queued now: a Draft left behind would only show twice.
        if let Err(error) = notes::remove_message(draft) {
            tracing::error!("removing the queued Draft failed: {error}");
        }

        Ok(path)
    }

    /// Gives `queued` back to `drafts/` as the Draft `text` it was, `notes`
    /// beside it holding the server's answer.
    pub fn give_back(&self, queued: &Queued, text: &str, notes: &Notes) -> io::Result<()> {
        // A Draft's file ends in `.eml`, whatever claim the Message was under.
        let name = queued.path.with_extension("eml");
        let draft = self
            .dir("drafts")?
            .join(name.file_name().unwrap_or_default());
        notes.write(&draft, MODE)?;
        atomic_file::write(&draft, text.as_bytes(), MODE)?;
        queued.remove()
    }

    /// Takes the Message queued at `path` back to `drafts/`, unless SMTP
    /// has it already: `None` then.
    pub fn take_back(&self, path: &Path) -> io::Result<Option<TakenBack>> {
        // The rename is the claim: the sync connection takes the file by one too.
        let taken = path.with_extension("undone");
        match std::fs::rename(path, &taken) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => result?,
        }

        let restored = self.restore(path, &taken);
        if restored.is_err()
            && let Err(error) = std::fs::rename(&taken, path)
        {
            tracing::error!("putting {} back failed: {error}", path.display());
        }

        restored.map(Some)
    }

    /// The Draft the Message `taken` was, written back to `drafts/`.
    fn restore(&self, path: &Path, taken: &Path) -> io::Result<TakenBack> {
        let notes = Notes::read(taken)?.unwrap_or_default();
        let draft = self
            .dir("drafts")?
            .join(path.file_name().unwrap_or_default());
        let text = notes.draft.unwrap_or_default();
        let notes = DraftNotes {
            account: notes.account,
            answering: notes.answering,
        };

        draft_file::restore(&draft, &text, &notes)?;
        if let Err(error) = notes::remove_message(taken) {
            tracing::error!("removing a Send taken back failed: {error}");
        }

        Ok(TakenBack {
            path: draft,
            text,
            notes,
        })
    }
}

impl Queued {
    /// The time it is held until, by its notes.
    pub fn hold_until(&self) -> Option<DateTime<Utc>> {
        self.notes.as_ref()?.hold_until
    }

    /// Claims the Message for SMTP by a rename, unless an Undo took it
    /// first: false then.
    pub fn claim(&mut self) -> io::Result<bool> {
        if self.path.extension().and_then(OsStr::to_str) != Some("eml") {
            return Ok(true);
        }

        let sending = self.path.with_extension("sending");
        match std::fs::rename(&self.path, &sending) {
            Ok(()) => {
                self.path = sending;
                Ok(true)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// Hands a claimed Message SMTP did not take back to the Outbox, where
    /// an Undo may take it.
    pub fn unclaim(&mut self) -> io::Result<()> {
        let waiting = self.path.with_extension("eml");
        std::fs::rename(&self.path, &waiting)?;
        self.path = waiting;
        Ok(())
    }

    /// Marks the Message accepted by SMTP, so it is never sent again.
    pub fn mark_sent(&mut self) -> io::Result<()> {
        let sent = self.path.with_extension("sent");
        std::fs::rename(&self.path, &sent)?;
        (self.path, self.sent) = (sent, true);
        Ok(())
    }

    /// Keeps `error`, why the last Send failed, in the Message's `notes`.
    pub fn note_failure(&self, notes: Notes, error: String) -> io::Result<()> {
        let notes = Notes {
            error: Some(error),
            ..notes
        };

        notes.write(&self.path, MODE)
    }

    /// Takes the Message out of the Outbox, with its notes.
    pub fn remove(&self) -> io::Result<()> {
        notes::remove_message(&self.path)
    }
}
