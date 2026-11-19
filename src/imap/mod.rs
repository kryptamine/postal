//! The IMAP boundary. Postal writes only `\Seen`, `\Answered` and the moves of
//! Delete and Undo; bodies are fetched with `BODY.PEEK`.

mod connect;
mod error;
mod moved_uids;
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
