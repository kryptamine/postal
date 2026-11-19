use async_imap::{
    imap_proto::{AttributeValue, MailboxDatum, Response},
    types::UnsolicitedResponse,
};

use crate::message_flags::MessageFlags;

/// A change the server announced on its own, by sequence number.
pub enum Pushed {
    /// The Inbox now holds this many Messages.
    Exists(u32),
    Expunge(u32),
    Flags {
        sequence: u32,
        /// Sent only by some servers.
        uid: Option<u32>,
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
            Response::Fetch(sequence, ref attributes) => {
                let flags = attributes.iter().find_map(|a| match a {
                    AttributeValue::Flags(flags) => Some(flags),
                    _ => None,
                })?;

                let uid = attributes.iter().find_map(|a| match a {
                    AttributeValue::Uid(uid) => Some(*uid),
                    _ => None,
                });

                Some(Self::Flags {
                    sequence,
                    uid,
                    flags: MessageFlags::from_names(flags.iter().map(|f| &**f)),
                })
            }
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
