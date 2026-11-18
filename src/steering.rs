use std::sync::Arc;

use futures::{FutureExt, StreamExt, channel::mpsc::UnboundedReceiver, stream::Peekable};

use crate::folder::INBOX;

/// What the screen asks of the sync connection.
pub enum Ask {
    /// Resync and IDLE on this Folder from now on.
    Show(Arc<str>),
    /// `LIST` and `STATUS (UNSEEN)`, for the Folder Picker.
    ListFolders,
}

/// What the asks waiting came to.
#[derive(Default)]
pub struct Asked {
    pub switched: bool,
    pub list: bool,
}

/// The Folder the sync connection follows, and the screen's asks of it.
/// It outlives each connection, so a reconnect resumes on the same Folder.
pub struct Steering {
    pub folder: Arc<str>,
    asks: Peekable<UnboundedReceiver<Ask>>,
}

impl Steering {
    pub fn new(asks: UnboundedReceiver<Ask>) -> Self {
        Self {
            folder: INBOX.into(),
            asks: asks.peekable(),
        }
    }

    /// Takes every ask waiting, the last Folder shown winning; `None` once
    /// the screen has stopped asking.
    pub fn take(&mut self) -> Option<Asked> {
        let mut asked = Asked::default();
        loop {
            match self.asks.next().now_or_never() {
                None => return Some(asked),
                Some(None) => return None,
                Some(Some(Ask::Show(folder))) => {
                    asked.switched |= folder != self.folder;
                    self.folder = folder;
                }
                Some(Some(Ask::ListFolders)) => asked.list = true,
            }
        }
    }

    /// Resolves once an ask waits, or the screen has stopped asking.
    pub async fn asking(&mut self) {
        std::pin::Pin::new(&mut self.asks).peek().await;
    }
}
