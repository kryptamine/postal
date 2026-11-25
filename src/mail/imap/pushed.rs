use async_imap::{
    imap_proto::{MailboxDatum, Response},
    types::UnsolicitedResponse,
};

use super::attributes;
use crate::{domain::message_flags::MessageFlags, domain::uid::Uid};

/// A change the server announced on its own, by sequence number.
pub enum Pushed {
    /// The Inbox now holds this many Messages.
    Exists(u32),
    Expunge(u32),
    Flags {
        sequence: u32,
        /// Sent only by some servers.
        uid: Option<Uid>,
        flags: MessageFlags,
    },
    /// The library's queue filled up and may have dropped announcements:
    /// the sequence numbers no longer say which Message is which.
    Lost,
}

impl Pushed {
    /// `None` for a response that changes nothing Postal shows.
    pub fn from_response(response: &Response<'_>) -> Option<Self> {
        match *response {
            Response::MailboxData(MailboxDatum::Exists(count)) => Some(Self::Exists(count)),
            Response::Expunge(sequence) => Some(Self::Expunge(sequence)),
            Response::Fetch(sequence, ref answer) => Some(Self::Flags {
                sequence,
                uid: attributes::uid(answer),
                flags: attributes::flags(answer)?,
            }),
            _ => None,
        }
    }

    pub fn from_unsolicited(response: &UnsolicitedResponse) -> Option<Self> {
        match response {
            UnsolicitedResponse::Exists(count) => Some(Self::Exists(*count)),
            UnsolicitedResponse::Expunge(sequence) => Some(Self::Expunge(*sequence)),
            UnsolicitedResponse::Other(response) => Self::from_response(response.parsed()),
            _ => None,
        }
    }
}
