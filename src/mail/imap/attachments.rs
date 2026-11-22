use async_imap::imap_proto::{
    BodyContentCommon, BodyContentSinglePart, BodyStructure, ContentEncoding,
};

use super::{encoding, file_name};
use crate::{
    domain::attachment::Attachments, domain::transfer_encoding::TransferEncoding, mail::header,
};

/// Base64 as mail sends it: lines of 76 digits and a CRLF, 57 octets each.
const BASE64_LINE: u64 = 78;

/// A part with `Content-Disposition: attachment`, or a non-text part with
/// a file name. A part with a `Content-ID` and no such disposition is
/// taken for an inline image, and an attached Message is not entered.
pub fn find(structure: &BodyStructure<'_>) -> Attachments {
    let mut found = Attachments::default();
    collect(structure, &[], &mut |_, common, other, name| {
        found.push(&name.unwrap_or_else(|| mime(common)), size(other));
    });

    found
}

/// The part number and transfer encoding of the attachment at `index` in
/// `find`'s order; a Message that is not multipart is its own part 1.
pub fn part(structure: &BodyStructure<'_>, index: usize) -> Option<(Vec<u32>, TransferEncoding)> {
    let (mut seen, mut picked) = (0, None);
    collect(structure, &[], &mut |path, _, other, _| {
        if seen == index {
            let path = if path.is_empty() {
                vec![1]
            } else {
                path.to_vec()
            };
            picked = Some((path, encoding::of(&other.transfer_encoding)));
        }

        seen += 1;
    });

    picked
}

/// What `found` is handed of each attachment under `structure`, whose part
/// number is `path`: empty at the top.
type Found<'f> =
    dyn FnMut(&[u32], &BodyContentCommon<'_>, &BodyContentSinglePart<'_>, Option<String>) + 'f;

fn collect(structure: &BodyStructure<'_>, path: &[u32], found: &mut Found<'_>) {
    match structure {
        BodyStructure::Multipart { bodies, .. } => {
            for (index, body) in (1..).zip(bodies) {
                collect(body, &[path, &[index]].concat(), found);
            }
        }
        BodyStructure::Message {
            common,
            other,
            envelope,
            ..
        } => {
            let subject = || {
                envelope
                    .subject
                    .as_deref()
                    .map(header::decode)
                    .filter(|s| !s.trim().is_empty())
            };
            found(path, common, other, name(common).or_else(subject));
        }
        BodyStructure::Text { common, other, .. } => {
            if attached(common) {
                found(path, common, other, name(common));
            }
        }
        BodyStructure::Basic { common, other, .. } => {
            let name = name(common);
            if attached(common) || (name.is_some() && other.id.is_none()) {
                found(path, common, other, name);
            }
        }
    }
}

pub fn attached(common: &BodyContentCommon<'_>) -> bool {
    common
        .disposition
        .as_ref()
        .is_some_and(|d| d.ty.eq_ignore_ascii_case("attachment"))
}

/// The disposition's `filename`, else the `Content-Type`'s `name`.
fn name(common: &BodyContentCommon<'_>) -> Option<String> {
    let disposition = common
        .disposition
        .as_ref()
        .and_then(|d| d.params.as_deref());
    disposition
        .and_then(|params| file_name::find(params, "filename"))
        .or_else(|| file_name::find(common.ty.params.as_deref()?, "name"))
}

fn mime(common: &BodyContentCommon<'_>) -> String {
    format!("{}/{}", common.ty.ty, common.ty.subtype).to_lowercase()
}

/// Base64 carries three octets in four digits, after the line breaks.
fn size(other: &BodyContentSinglePart<'_>) -> u64 {
    let sent = u64::from(other.octets);
    match other.transfer_encoding {
        ContentEncoding::Base64 => (sent - 2 * (sent / BASE64_LINE)) * 3 / 4,
        _ => sent,
    }
}
