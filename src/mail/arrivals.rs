use std::cell::Cell;

use crate::{cache::Changes, domain::folder_name::FolderName, mail::arrival::Arrival};

/// Which new Inbox rows are arrivals. It outlives each connection, so a
/// reconnect's resync still tells of the mail that came during the outage.
pub struct Arrivals {
    synced: Cell<bool>,
}

impl Arrivals {
    pub const fn new() -> Self {
        Self {
            synced: Cell::new(false),
        }
    }

    /// The new Unread rows of `changes` to `folder` not sent by the Account,
    /// when `folder` is the Inbox. The run's first resync of it finds mail
    /// that came while Postal was closed: none of that is an arrival.
    pub fn of(&self, folder: &FolderName, changes: &Changes) -> Vec<Arrival> {
        if *folder != *FolderName::inbox() {
            return Vec::new();
        }

        let first = !self.synced.replace(true);
        if first || changes.replace {
            return Vec::new();
        }

        let rows = changes.new.iter().map(|new| &new.row);
        rows.filter(|row| row.unread && !row.from_me)
            .map(|row| Arrival {
                sender: row.sender.clone(),
                subject: row.subject.clone(),
            })
            .collect()
    }
}
