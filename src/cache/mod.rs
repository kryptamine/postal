//! The Cache: `SQLite` in WAL mode, one row per Message keyed by UID, with
//! the Inbox's UIDVALIDITY beside them.

mod changes;
mod error;
mod schema;
mod store;

pub use changes::Changes;
pub use error::Error;
pub use schema::delete;
pub use store::Cache;
