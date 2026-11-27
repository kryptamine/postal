use std::io;

use crate::{cache, mail::sync};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("terminal: {0}")]
    Terminal(io::Error),
    #[error(transparent)]
    Cache(#[from] cache::Error),
    #[error(transparent)]
    Sync(#[from] sync::Error),
}
