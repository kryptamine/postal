use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
};

use crate::{
    atomic_file,
    notes::{self, Notes},
};

/// Only the user may read their own writing.
const MODE: u32 = 0o600;

/// The Messages sent and not yet accepted, in `outbox/`: each as it goes
/// out in `<id>.eml`, renamed `<id>.sent` once SMTP accepts it and until
/// its copy is in Sent, its notes in `<id>.json` (ADR 0013).
pub struct Outbox {
    /// `None` without a data directory: nothing can be queued then.
    data: Option<PathBuf>,
}

/// One Message in the Outbox.
pub struct Queued {
    /// Its file.
    pub path: PathBuf,
    /// Accepted by SMTP; only its copy in Sent is left.
    pub sent: bool,
}

impl Outbox {
    /// The Outbox under `data`, beside `drafts/`.
    pub const fn new(data: Option<PathBuf>) -> Self {
        Self { data }
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
                Some("eml") => false,
                Some("sent") => true,
                _ => continue,
            };

            queued.push(Queued { path, sent });
        }

        // A file's name starts with the time its Draft was made.
        queued.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(queued)
    }

    /// Moves the Draft at `draft` here as `message`, its notes beside it.
    pub fn queue(&self, draft: &Path, message: &[u8], notes: &Notes) -> io::Result<()> {
        let path = self
            .dir("outbox")?
            .join(draft.file_name().unwrap_or_default());
        // The notes first: a Message is in the Outbox once its file is.
        notes.write(&path, MODE)?;
        atomic_file::write(&path, message, MODE)?;
        // Queued now: a Draft left behind would only show twice.
        if let Err(error) = notes::remove_message(draft) {
            tracing::error!("removing the queued Draft failed: {error}");
        }

        Ok(())
    }

    /// Gives `queued` back to `drafts/` as the Draft `text` it was, `notes`
    /// beside it holding the server's answer.
    pub fn give_back(&self, queued: &Queued, text: &str, notes: &Notes) -> io::Result<()> {
        let draft = self
            .dir("drafts")?
            .join(queued.path.file_name().unwrap_or_default());
        notes.write(&draft, MODE)?;
        atomic_file::write(&draft, text.as_bytes(), MODE)?;
        queued.remove()
    }
}

impl Queued {
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
