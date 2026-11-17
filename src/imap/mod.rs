//! The IMAP boundary. Postal only reads: the Inbox is opened with `EXAMINE`
//! and nothing is ever stored, copied or expunged.

mod connect;
mod error;
mod session;
mod traced;
mod wire_trace;

pub use error::Error;
pub use session::{ImapSession, MailServer};
