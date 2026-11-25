//! The IMAP boundary. Postal writes only `\Seen`, `\Answered` and the moves of
//! Delete and Undo; bodies are fetched with `BODY.PEEK`.

mod attachments;
mod attributes;
mod bodies;
mod body_plan;
mod connect;
mod encoding;
mod error;
mod file_name;
mod login;
mod moved;
mod moved_uids;
mod named_roles;
mod new_messages;
mod opening;
mod origin;
mod pushed;
mod session;
mod tagged;
mod text_part;
mod traced;
mod wire_trace;
mod xoauth2;

pub use error::Error;
pub use moved::Moved;
pub use opening::{Mode, Opening};
pub use pushed::Pushed;
pub use session::{Examined, ImapSession, MailServer};
