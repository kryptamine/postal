use std::{collections::HashMap, fmt::Write};

use async_imap::imap_proto::{BodyContentCommon, BodyStructure, MessageSection, SectionPath};

use super::attachments::attached;

/// What of a Message the Preview fetches: its header and its inline text
/// parts, with the MIME headers of the multiparts around them, put back
/// together as a smaller Message (docs/adr/0024-the-preview-fetches-text-parts.md).
pub enum Plan {
    /// `BODY[]` whole: a multipart without a boundary cannot be put back.
    Whole,
    /// The header, then the body of the part kept, if any.
    Parts(Option<Node>),
}

/// A part kept under the header.
pub enum Node {
    Text,
    Multipart {
        boundary: String,
        /// Each by its part number.
        children: Vec<(Vec<u32>, Self)>,
    },
}

/// What one fetch answered of one Message, by section as `key` names it.
pub type Sections = HashMap<String, Vec<u8>>;

/// The section `BODY[]` answers to.
pub const WHOLE: &str = "";
const HEADER: &str = "HEADER";

impl Plan {
    pub fn of(structure: &BodyStructure<'_>) -> Self {
        node(structure, &[]).map_or(Self::Whole, Self::Parts)
    }

    /// The fetch items, `UID` first.
    pub fn items(&self) -> String {
        let Self::Parts(top) = self else {
            return "(UID BODY.PEEK[])".to_owned();
        };

        let mut items = format!("(UID BODY.PEEK[{HEADER}]");
        if let Some(top) = top {
            push_items(&mut items, top, &[]);
        }

        items.push(')');
        items
    }

    /// The Message as `sections` holds it; a section missing reads as empty.
    pub fn assemble(&self, sections: &Sections) -> Vec<u8> {
        let get = |key: &str| sections.get(key).map_or(&[][..], Vec::as_slice);
        let Self::Parts(top) = self else {
            return get(WHOLE).to_vec();
        };

        let mut out = Vec::new();
        header(&mut out, get(HEADER));
        if let Some(top) = top {
            body(&mut out, top, &[], &get);
        }

        out
    }
}

/// The section a fetch answered, as `Plan` names it; `None` for one it
/// never asks for.
pub fn key(answered: Option<&SectionPath>) -> Option<String> {
    match answered {
        None => Some(WHOLE.to_owned()),
        Some(SectionPath::Full(MessageSection::Header)) => Some(HEADER.to_owned()),
        Some(SectionPath::Part(path, None)) => Some(section(path)),
        Some(SectionPath::Part(path, Some(MessageSection::Mime))) => Some(mime(path)),
        Some(_) => None,
    }
}

/// The part kept of `structure`, at `path`; `Err` for a multipart without
/// a boundary. Attachments and parts other than text are left out, but for
/// a calendar part, which the invitation card reads.
fn node(structure: &BodyStructure<'_>, path: &[u32]) -> Result<Option<Node>, ()> {
    match structure {
        BodyStructure::Text { common, .. } if !attached(common) || calendar(common) => {
            Ok(Some(Node::Text))
        }
        BodyStructure::Basic { common, .. } if calendar(common) => Ok(Some(Node::Text)),
        BodyStructure::Multipart { common, bodies, .. } => {
            let boundary = common
                .ty
                .params
                .iter()
                .flatten()
                .find(|(key, _)| key.eq_ignore_ascii_case("boundary"))
                .map(|(_, value)| value.to_string())
                .ok_or(())?;
            let mut children = Vec::new();
            for (index, body) in (1..).zip(bodies) {
                let child = [path, &[index]].concat();
                if let Some(node) = node(body, &child)? {
                    children.push((child, node));
                }
            }

            Ok((!children.is_empty()).then_some(Node::Multipart { boundary, children }))
        }
        _ => Ok(None),
    }
}

/// `text/calendar`, or `application/ics` as some senders name it.
fn calendar(common: &BodyContentCommon<'_>) -> bool {
    let (ty, subtype) = (&common.ty.ty, &common.ty.subtype);
    (ty.eq_ignore_ascii_case("text") && subtype.eq_ignore_ascii_case("calendar"))
        || (ty.eq_ignore_ascii_case("application") && subtype.eq_ignore_ascii_case("ics"))
}

fn push_items(items: &mut String, node: &Node, path: &[u32]) {
    match node {
        // Writing to a String cannot fail.
        Node::Text => write!(items, " BODY.PEEK[{}]", section(path)).unwrap_or_default(),
        Node::Multipart { children, .. } => {
            for (child, node) in children {
                write!(items, " BODY.PEEK[{}]", mime(child)).unwrap_or_default();
                push_items(items, node, child);
            }
        }
    }
}

fn body<'a>(out: &mut Vec<u8>, node: &Node, path: &[u32], get: &impl Fn(&str) -> &'a [u8]) {
    match node {
        Node::Text => out.extend_from_slice(get(&section(path))),
        Node::Multipart { boundary, children } => {
            for (child, node) in children {
                out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                header(out, get(&mime(child)));
                body(out, node, child, get);
                out.extend_from_slice(b"\r\n");
            }

            out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        }
    }
}

/// Appends `fetched` ending in the one blank line that closes a header,
/// whether or not the server sent it.
fn header(out: &mut Vec<u8>, fetched: &[u8]) {
    let end = fetched
        .iter()
        .rposition(|&b| b != b'\r' && b != b'\n')
        .map_or(0, |last| last + 1);
    if end > 0 {
        out.extend_from_slice(&fetched[..end]);
        out.extend_from_slice(b"\r\n");
    }

    out.extend_from_slice(b"\r\n");
}

/// A part number as a section names it; the top of a Message that is not
/// multipart is part 1 all the same.
pub fn section(path: &[u32]) -> String {
    if path.is_empty() {
        return "1".to_owned();
    }

    path.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn mime(path: &[u32]) -> String {
    format!("{}.MIME", section(path))
}
