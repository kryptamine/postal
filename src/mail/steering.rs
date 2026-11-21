use futures::{FutureExt, StreamExt, channel::mpsc::UnboundedReceiver, stream::Peekable};

use crate::{domain::folder::INBOX, domain::folder_name::FolderName, domain::thread_id::ThreadId};

/// What the screen asks of the sync connection.
pub enum Ask {
    /// Resync and IDLE on this Folder from now on.
    Show(FolderName),
    /// `LIST` and `STATUS (UNSEEN)`, for the Folder Picker.
    ListFolders,
    /// The Messages of Gmail's Thread with this ID in other Folders.
    Thread(ThreadId),
    /// The Cache holds flag changes or moves for the server.
    Changes,
    /// The Outbox holds a Message to send.
    Send,
}

/// What the asks waiting came to.
#[derive(Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each is an ask of its own, taken in the order the loop picks"
)]
pub struct Asked {
    /// The shown Folder changed, or a resync of it was asked.
    pub switched: bool,
    pub list: bool,
    /// The Thread last asked for, when the Messages of one were.
    pub thread: Option<ThreadId>,
    pub changes: bool,
    pub send: bool,
}

impl Asked {
    /// Nothing but a Folder switch: the Outbox is retried on these, and
    /// not ahead of every Mark Read or Folder Picker.
    pub const fn quiet(&self) -> bool {
        !self.changes && !self.list && self.thread.is_none()
    }
}

/// The Folder the sync connection follows, and the screen's asks of it.
/// It outlives each connection, so a reconnect resumes on the same Folder.
pub struct Steering {
    pub folder: FolderName,
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
                Some(Some(Ask::Thread(id))) => asked.thread = Some(id),
                Some(Some(Ask::Changes)) => asked.changes = true,
                Some(Some(Ask::Send)) => asked.send = true,
            }
        }
    }

    /// Resolves once an ask waits, or the screen has stopped asking.
    pub async fn asking(&mut self) {
        std::pin::Pin::new(&mut self.asks).peek().await;
    }
}
