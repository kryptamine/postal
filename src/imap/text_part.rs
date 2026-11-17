use async_imap::imap_proto::{
    BodyContentCommon, BodyContentSinglePart, BodyStructure, ContentEncoding,
};

use crate::{text_part::TextPart, transfer_encoding::TransferEncoding};

/// The first inline `text/plain` part, else the first inline `text/html`;
/// `None` when the Message has neither. Attached Messages are not entered.
pub fn find(structure: &BodyStructure<'_>) -> Option<TextPart> {
    let mut parts = Vec::new();
    collect(structure, None, &mut parts);
    let plain = parts.iter().position(|part| !part.html);
    let chosen = plain.or_else(|| (!parts.is_empty()).then_some(0))?;
    Some(parts.swap_remove(chosen))
}

/// Appends the inline text parts under `structure`, whose part number is
/// `section`; `None` at the top level.
fn collect(structure: &BodyStructure<'_>, section: Option<&str>, parts: &mut Vec<TextPart>) {
    match structure {
        BodyStructure::Text { common, other, .. } => {
            // A Message that is not multipart has a part 1 all the same.
            if let Some(part) = text(common, other, section.unwrap_or("1")) {
                parts.push(part);
            }
        }
        BodyStructure::Multipart { bodies, .. } => {
            for (index, body) in (1..).zip(bodies) {
                let child = section.map_or_else(
                    || format!("{index}"),
                    |section| format!("{section}.{index}"),
                );
                collect(body, Some(&child), parts);
            }
        }
        BodyStructure::Basic { .. } | BodyStructure::Message { .. } => {}
    }
}

fn text(
    common: &BodyContentCommon<'_>,
    other: &BodyContentSinglePart<'_>,
    section: &str,
) -> Option<TextPart> {
    let subtype = &common.ty.subtype;
    let html = if subtype.eq_ignore_ascii_case("plain") {
        false
    } else if subtype.eq_ignore_ascii_case("html") {
        true
    } else {
        return None;
    };

    if common
        .disposition
        .as_ref()
        .is_some_and(|d| d.ty.eq_ignore_ascii_case("attachment"))
    {
        return None;
    }

    let charset = common
        .ty
        .params
        .iter()
        .flatten()
        .find(|(key, _)| key.eq_ignore_ascii_case("charset"))
        .map(|(_, value)| value.to_string())
        .unwrap_or_default();
    let encoding = match &other.transfer_encoding {
        ContentEncoding::QuotedPrintable => TransferEncoding::QuotedPrintable,
        ContentEncoding::Base64 => TransferEncoding::Base64,
        ContentEncoding::Other(label) => TransferEncoding::from_label(label),
        ContentEncoding::SevenBit | ContentEncoding::EightBit | ContentEncoding::Binary => {
            TransferEncoding::Identity
        }
    };

    Some(TextPart {
        section: section.to_owned(),
        encoding,
        charset,
        html,
    })
}
