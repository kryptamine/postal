use std::{io, path::PathBuf};

use crate::{answering::Answering, draft::Draft};

/// A Draft shown after the editor exits, for `y` to send, `e` to edit again
/// or `q` to discard.
pub struct DraftPreview {
    pub path: PathBuf,
    /// The file as written, kept for an Undo of its discard.
    pub text: String,
    /// The error says why `y` is refused.
    pub draft: Result<Draft, String>,
    /// The last Send's error, such as the server's answer.
    pub failure: Option<String>,
    pub sending: bool,
    /// For a Reply.
    pub answering: Option<Answering>,
}

impl DraftPreview {
    pub fn read(path: PathBuf, answering: Option<Answering>) -> io::Result<Self> {
        let text = std::fs::read_to_string(&path)?;
        Ok(Self::new(path, text, answering))
    }

    pub fn new(path: PathBuf, text: String, answering: Option<Answering>) -> Self {
        Self {
            draft: Draft::parse(&text),
            path,
            text,
            failure: None,
            sending: false,
            answering,
        }
    }
}
