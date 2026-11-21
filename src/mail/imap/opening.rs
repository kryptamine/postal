use crate::domain::folder_name::FolderName;

/// How a Folder is opened: `Select` lets `store` and `move_to` change it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Examine,
    Select,
}

/// One open of a Folder on the wire, equal only to itself: what the server
/// announces belongs to the last.
#[derive(Clone, PartialEq, Eq)]
pub struct Opening {
    folder: FolderName,
    mode: Mode,
    serial: u64,
}

impl Opening {
    pub const fn new(folder: FolderName, mode: Mode, serial: u64) -> Self {
        Self {
            folder,
            mode,
            serial,
        }
    }

    pub const fn folder(&self) -> &FolderName {
        &self.folder
    }

    /// Whether `folder` opened as `mode` asks nothing more of the server.
    pub fn covers(&self, folder: &FolderName, mode: Mode) -> bool {
        self.folder == *folder && (self.mode == Mode::Select || mode == Mode::Examine)
    }
}
