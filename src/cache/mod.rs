//! The Cache: `SQLite` in WAL mode, one row per Message keyed by Folder
//! and UID, with each Folder's UIDVALIDITY beside them.

mod bodies;
mod changes;
mod correspondence;
mod envelope_pass;
mod error;
mod flags;
mod folder_key;
mod folders;
mod forget;
mod list;
mod migrations;
mod moves;
mod reader;
mod schema;
mod snippets;
mod sql_values;
mod state;
mod store;
mod threads;

pub use changes::Changes;
pub use correspondence::SentPass;
pub use envelope_pass::EnvelopePass;
pub use error::Error;
pub use forget::forget;
pub use moves::{Destination, PendingMove};
pub use reader::{Reader, Seen, ThreadMessage};
pub use schema::delete;
pub use store::Cache;
