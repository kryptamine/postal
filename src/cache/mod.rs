//! The Cache: `SQLite` in WAL mode, one row per Message keyed by Folder
//! and UID, with each Folder's UIDVALIDITY beside them.

mod changes;
mod correspondence;
mod error;
mod forget;
mod moves;
mod reader;
mod schema;
mod store;

pub use changes::Changes;
pub use correspondence::SentPass;
pub use error::Error;
pub use forget::forget;
pub use moves::{Destination, PendingMove};
pub use reader::{Kept, Reader, Seen};
pub use schema::delete;
pub use store::Cache;
