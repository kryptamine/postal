//! What Postal reads of one `FETCH` answer's attributes.

use async_imap::imap_proto::{AttributeValue, BodyStructure, SectionPath};

use crate::{domain::message_flags::MessageFlags, domain::uid::Uid};

pub fn uid(attributes: &[AttributeValue<'_>]) -> Option<Uid> {
    attributes.iter().find_map(|a| match a {
        AttributeValue::Uid(uid) => Some(Uid::new(*uid)),
        _ => None,
    })
}

/// `None` when the answer carries no `FLAGS`.
pub fn flags(attributes: &[AttributeValue<'_>]) -> Option<MessageFlags> {
    attributes.iter().find_map(|a| match a {
        AttributeValue::Flags(flags) => Some(MessageFlags::from_names(flags.iter().map(|f| &**f))),
        _ => None,
    })
}

pub fn section<'a>(attributes: &'a [AttributeValue<'_>], path: &SectionPath) -> Option<&'a [u8]> {
    attributes.iter().find_map(|a| match a {
        AttributeValue::BodySection {
            section: Some(section),
            data: Some(data),
            ..
        } if section == path => Some(data.as_ref()),
        _ => None,
    })
}

pub fn structure<'a>(attributes: &'a [AttributeValue<'_>]) -> Option<&'a BodyStructure<'a>> {
    attributes.iter().find_map(|a| match a {
        AttributeValue::BodyStructure(structure) => Some(structure),
        _ => None,
    })
}
