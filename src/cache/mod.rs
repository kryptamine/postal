//! The Cache: `SQLite` in WAL mode, one row per Message keyed by Folder
//! and UID, with each Folder's UIDVALIDITY beside them.

mod changes;
mod error;
mod forget;
mod moves;
mod schema;
mod store;

pub use changes::Changes;
pub use error::Error;
pub use forget::forget;
pub use moves::{Destination, PendingMove};
pub use schema::delete;
pub use store::Cache;
