use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{answering::Answering, atomic_file};

/// What an unsent Message's file cannot hold, in a JSON file beside it.
#[derive(Default, Serialize, Deserialize)]
pub struct Notes {
    /// The Draft's file as written, kept in the Outbox for a refusal to
    /// give back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<String>,
    /// The address of the Account it goes out from; the first Account's
    /// when it names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(default)]
    pub answering: Option<Answering>,
    /// The server's answer to the last Send.
    #[serde(default)]
    pub error: Option<String>,
}

impl Notes {
    /// `None` when the Message has no notes.
    pub fn read(message: &Path) -> io::Result<Option<Self>> {
        match std::fs::read(path(message)) {
            Ok(json) => serde_json::from_slice(&json)
                .map(Some)
                .map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn write(&self, message: &Path, mode: u32) -> io::Result<()> {
        let json = serde_json::to_vec(self).map_err(io::Error::other)?;
        atomic_file::write(&path(message), &json, mode)
    }
}

/// Removes the file `message` and its notes, if it has any.
pub fn remove_message(message: &Path) -> io::Result<()> {
    match std::fs::remove_file(path(message)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }

    std::fs::remove_file(message)
}

fn path(message: &Path) -> PathBuf {
    message.with_extension("json")
}
