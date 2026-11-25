//! The Cache: `SQLite` in WAL mode, one row per Message keyed by Folder
//! and UID, with each Folder's UIDVALIDITY beside them.

mod bodies;
mod body_index;
mod changes;
mod correspondence;
mod correspondents;
mod envelope_pass;
mod error;
mod flags;
mod folder_key;
mod folders;
mod folders_read;
mod forget;
mod listed;
mod message_row;
mod migrations;
mod moves;
mod query_sql;
mod reader;
mod received;
mod schema;
mod scope;
mod search;
mod snippets;
mod sql_values;
mod state;
mod store;
mod thread_rows;
mod threads;
mod window;

pub use body_index::Indexed;
pub use changes::Changes;
pub use correspondence::SentPass;
pub use correspondents::{Correspondent, Correspondents};
pub use envelope_pass::EnvelopePass;
pub use error::Error;
pub use forget::forget;
pub use listed::Listed;
pub use moves::{Destination, Goal, PendingMove};
pub use reader::{Reader, Seen, ThreadMessage};
pub use schema::{delete, settle};
pub use scope::Scope;
pub use search::{candidates, threads};
pub use store::Cache;
pub use window::Arrivals;
