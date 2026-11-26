use async_imap::imap_proto::BodyStructure;

use super::encoding;
use crate::domain::transfer_encoding::TransferEncoding;

/// A part found by its `Content-ID`.
pub struct InlinePart {
    pub path: Vec<u32>,
    pub id: String,
    pub kind: String,
    pub encoding: TransferEncoding,
}

/// The single parts under `structure` whose `Content-ID` is one of `ids`;
/// an attached Message is not entered.
pub fn find(structure: &BodyStructure<'_>, ids: &[String]) -> Vec<InlinePart> {
    let mut found = Vec::new();
    collect(structure, &[], ids, &mut found);
    found
}

fn collect(
    structure: &BodyStructure<'_>,
    path: &[u32],
    ids: &[String],
    found: &mut Vec<InlinePart>,
) {
    let (common, other) = match structure {
        BodyStructure::Multipart { bodies, .. } => {
            for (index, body) in (1..).zip(bodies) {
                collect(body, &[path, &[index]].concat(), ids, found);
            }

            return;
        }
        BodyStructure::Message { .. } => return,
        BodyStructure::Text { common, other, .. } | BodyStructure::Basic { common, other, .. } => {
            (common, other)
        }
    };

    let Some(id) = other.id.as_deref() else {
        return;
    };

    let id = id.trim().trim_start_matches('<').trim_end_matches('>');
    if !ids.iter().any(|wanted| wanted == id) {
        return;
    }

    found.push(InlinePart {
        // A Message that is not multipart is its own part 1.
        path: if path.is_empty() {
            vec![1]
        } else {
            path.to_vec()
        },
        id: id.to_owned(),
        // It lands inside an HTML attribute, so no quote gets through.
        kind: format!("{}/{}", common.ty.ty, common.ty.subtype)
            .to_lowercase()
            .replace(
                |c: char| !(c.is_ascii_alphanumeric() || "/.+-".contains(c)),
                "",
            ),
        encoding: encoding::of(&other.transfer_encoding),
    });
}
