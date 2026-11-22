use std::{io, path::PathBuf};

use crate::{
    compose::draft::Draft, compose::draft_notes::DraftNotes, compose::notes::Notes,
    domain::answering::Answering,
};

/// A Draft shown after the editor exits, for `y` to send, `e` to edit again,
/// `d` to keep or `q` to discard.
pub struct DraftPreview {
    pub path: PathBuf,
    /// The file as written, kept for an Undo of its discard.
    pub text: String,
    /// The error says why `y` is refused.
    pub draft: Result<Draft, String>,
    /// The last Send's error, such as the server's answer.
    pub failure: Option<String>,
    /// The address of the Account it goes out from; the first Account's
    /// when `None`.
    pub account: Option<String>,
    /// For a Reply.
    pub answering: Option<Answering>,
    /// An Outbox Message, shown read-only.
    pub queued: bool,
}

impl DraftPreview {
    /// The Draft's file, with the error and the answered Message its notes
    /// hold; `answering` wins over the notes'.
    pub fn read(path: PathBuf, answering: Option<Answering>) -> io::Result<Self> {
        let text = std::fs::read_to_string(&path)?;
        let notes = Notes::read(&path)?.unwrap_or_default();
        let mut preview = Self::new(
            path,
            text,
            DraftNotes {
                account: notes.account,
                answering: answering.or(notes.answering),
            },
        );
        preview.failure = notes.error;
        Ok(preview)
    }

    /// The Outbox Message at `path`, as its Draft was written, with the
    /// last Send's error.
    pub fn queued(path: PathBuf) -> io::Result<Self> {
        let notes = Notes::read(&path)?.unwrap_or_default();
        let text = notes.draft.unwrap_or_default();
        let mut preview = Self::new(
            path,
            text,
            DraftNotes {
                account: notes.account,
                answering: notes.answering,
            },
        );
        (preview.failure, preview.queued) = (notes.error, true);
        Ok(preview)
    }

    pub fn new(path: PathBuf, text: String, notes: DraftNotes) -> Self {
        Self {
            account: notes.account,
            draft: Draft::parse(&text).and_then(|draft| draft.attachable().map(|()| draft)),
            path,
            text,
            failure: None,
            answering: notes.answering,
            queued: false,
        }
    }
}
