//! The IMAP boundary. Postal only reads: each Folder is opened with `EXAMINE`
//! and nothing is ever stored, copied or expunged.

mod connect;
mod error;
mod new_messages;
mod pushed;
mod session;
mod tagged;
mod text_part;
mod traced;
mod whole_bodies;
mod wire_trace;
mod xoauth2;

pub use error::Error;
pub use pushed::Pushed;
pub use session::{Examined, ImapSession, MailServer};
