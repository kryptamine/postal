//! A Message body as the fake IMAP server serves it: its `BODYSTRUCTURE`,
//! and the octets of one part by part number.

use std::{fmt::Write, num::NonZeroU32};

use imap_next::imap_types::{
    body::{
        BasicFields, Body, BodyStructure, Disposition, MultiPartExtensionData,
        SinglePartExtensionData, SpecificFields,
    },
    core::{IString, NString, Vec1},
};
use mail_parser::{ContentType, Message, MessageParser, MimeHeaders, PartType};

#[derive(Clone)]
pub enum FakeBody {
    Part {
        media_type: &'static str,
        subtype: &'static str,
        charset: Option<&'static str>,
        /// The `Content-Transfer-Encoding`.
        encoding: &'static str,
        /// The part as sent, already in its transfer encoding.
        content: Vec<u8>,
        /// `Content-Disposition`, with its parameters as sent.
        disposition: Option<(&'static str, Vec<(String, String)>)>,
        /// `Content-Type` parameters beside the charset, as sent.
        params: Vec<(String, String)>,
        /// The `Content-ID`.
        id: Option<&'static str>,
    },
    Multipart {
        subtype: &'static str,
        parts: Vec<Self>,
    },
}

impl FakeBody {
    /// A `text/<subtype>` part in UTF-8.
    pub fn text(
        subtype: &'static str,
        encoding: &'static str,
        content: impl Into<Vec<u8>>,
    ) -> Self {
        Self::Part {
            media_type: "text",
            subtype,
            charset: Some("utf-8"),
            encoding,
            content: content.into(),
            disposition: None,
            params: Vec::new(),
            id: None,
        }
    }

    pub fn plain(content: &str) -> Self {
        Self::text("plain", "7bit", content)
    }

    pub const fn charset(mut self, name: &'static str) -> Self {
        if let Self::Part { charset, .. } = &mut self {
            *charset = Some(name);
        }

        self
    }

    pub fn attached(mut self) -> Self {
        if let Self::Part { disposition, .. } = &mut self {
            *disposition = Some(("attachment", Vec::new()));
        }

        self
    }

    /// Adds `key=value` to the `Content-Disposition`, as sent: `filename*`
    /// and its continuations carry RFC 2231 values.
    pub fn disposition_param(mut self, key: &str, value: &str) -> Self {
        if let Self::Part { disposition, .. } = &mut self {
            let (_, params) = disposition.get_or_insert(("attachment", Vec::new()));
            params.push((key.to_owned(), value.to_owned()));
        }

        self
    }

    /// Adds `key=value` to the `Content-Type`, as sent.
    pub fn type_param(mut self, key: &str, value: &str) -> Self {
        if let Self::Part { params, .. } = &mut self {
            params.push((key.to_owned(), value.to_owned()));
        }

        self
    }

    /// Without a disposition, or `inline`, and with `Content-ID: <cid>`.
    pub fn inline(mut self, cid: &'static str) -> Self {
        if let Self::Part {
            disposition, id, ..
        } = &mut self
        {
            *disposition = disposition.take().map(|(_, params)| ("inline", params));
            *id = Some(cid);
        }

        self
    }

    /// Without any `Content-Disposition`.
    pub fn undisposed(mut self) -> Self {
        if let Self::Part { disposition, .. } = &mut self {
            *disposition = None;
        }

        self
    }

    /// Base64 content of `lines` full lines of 76 digits, 57 octets each.
    pub fn lines(mut self, lines: usize) -> Self {
        if let Self::Part { content, .. } = &mut self {
            *content = [&b"A".repeat(76)[..], b"\r\n"].concat().repeat(lines);
        }

        self
    }

    pub fn file(media_type: &'static str, subtype: &'static str) -> Self {
        Self::Part {
            media_type,
            subtype,
            charset: None,
            encoding: "base64",
            content: b"AAECAwQF".to_vec(),
            disposition: Some(("attachment", Vec::new())),
            params: Vec::new(),
            id: None,
        }
    }

    pub const fn multipart(subtype: &'static str, parts: Vec<Self>) -> Self {
        Self::Multipart { subtype, parts }
    }

    /// The body of an `.eml` file, part by part, with each part's content
    /// kept in its transfer encoding.
    pub fn eml(raw: &[u8]) -> Self {
        let message = MessageParser::default().parse(raw).unwrap();
        Self::from_part(&message, 0)
    }

    fn from_part(message: &Message<'_>, id: u32) -> Self {
        let leak = |s: &str| -> &'static str { s.to_ascii_lowercase().leak() };
        let part = &message.parts[usize::try_from(id).unwrap()];
        let content_type = part.content_type();
        let media_type = content_type.map_or("text", |t| leak(t.ctype()));
        let subtype = content_type.and_then(|t| t.subtype()).map_or("plain", leak);
        if let PartType::Multipart(children) = &part.body {
            let parts = children
                .iter()
                .map(|&child| Self::from_part(message, child))
                .collect();
            return Self::Multipart { subtype, parts };
        }

        let raw = message.raw_message();
        let start = usize::try_from(part.raw_body_offset()).unwrap();
        let end = usize::try_from(part.raw_end_offset()).unwrap();
        let content = &raw[start..end];
        Self::Part {
            media_type,
            subtype,
            charset: content_type.and_then(|t| t.attribute("charset")).map(leak),
            encoding: part.content_transfer_encoding().map_or("7bit", leak),
            content: content.to_vec(),
            disposition: part
                .content_disposition()
                .is_some_and(ContentType::is_attachment)
                .then(|| ("attachment", Vec::new())),
            params: Vec::new(),
            id: None,
        }
    }

    pub fn structure(&self) -> BodyStructure<'static> {
        self.structure_at("1")
    }

    /// The structure of this part, `id` naming its boundary as `content` does.
    fn structure_at(&self, id: &str) -> BodyStructure<'static> {
        let string = |s: &str| IString::try_from(s.to_owned()).unwrap();
        match self {
            Self::Part { .. } => self.single_structure(),
            Self::Multipart { subtype, parts } => BodyStructure::Multi {
                bodies: Vec1::try_from(
                    (1..)
                        .zip(parts)
                        .map(|(index, part)| part.structure_at(&format!("{id}.{index}")))
                        .collect::<Vec<_>>(),
                )
                .unwrap(),
                subtype: string(subtype),
                extension_data: Some(MultiPartExtensionData {
                    parameter_list: vec![(string("boundary"), string(&boundary(id)))],
                    tail: None,
                }),
            },
        }
    }

    /// The structure of a part that is not multipart.
    fn single_structure(&self) -> BodyStructure<'static> {
        let string = |s: &str| IString::try_from(s.to_owned()).unwrap();
        let Self::Part {
            media_type,
            subtype,
            charset,
            encoding,
            content,
            disposition,
            params,
            id,
        } = self
        else {
            panic!("a multipart has a structure of its own")
        };
        let basic = BasicFields {
            parameter_list: charset
                .iter()
                .map(|c| (string("charset"), string(c)))
                .chain(params.iter().map(|(k, v)| (string(k), string(v))))
                .collect(),
            id: NString(id.map(|id| string(&format!("<{id}>")))),
            description: NString(None),
            content_transfer_encoding: string(encoding),
            size: u32::try_from(content.len()).unwrap(),
        };

        let specific = if *media_type == "text" {
            SpecificFields::Text {
                subtype: string(subtype),
                number_of_lines: u32::try_from(content.split(|&b| b == b'\n').count()).unwrap(),
            }
        } else {
            SpecificFields::Basic {
                r#type: string(media_type),
                subtype: string(subtype),
            }
        };

        let extension_data = disposition
            .as_ref()
            .map(|(kind, params)| SinglePartExtensionData {
                md5: NString(None),
                tail: Some(Disposition {
                    disposition: Some((
                        string(kind),
                        params.iter().map(|(k, v)| (string(k), string(v))).collect(),
                    )),
                    tail: None,
                }),
            });

        BodyStructure::Single {
            body: Body { basic, specific },
            extension_data,
        }
    }

    /// The Message as `BODY[]` serves it: `headers`, each ending in CRLF,
    /// then this body in MIME.
    pub fn message(&self, headers: &str) -> Vec<u8> {
        let mut message = self.header(headers).into_bytes();
        self.content(&mut message, "1");
        message
    }

    /// The Message's header as `BODY[HEADER]` serves it, the blank line
    /// after it included.
    pub fn header(&self, headers: &str) -> String {
        format!("{headers}MIME-Version: 1.0\r\n{}\r\n", self.headers("1"))
    }

    /// This part's own headers, each ending in CRLF.
    fn headers(&self, id: &str) -> String {
        match self {
            Self::Part {
                media_type,
                subtype,
                charset,
                encoding,
                disposition,
                params,
                id,
                ..
            } => {
                let charset = charset
                    .map(|c| format!("; charset={c}"))
                    .unwrap_or_default();
                let mut headers = format!(
                    "Content-Type: {media_type}/{subtype}{charset}{}\r\nContent-Transfer-Encoding: {encoding}\r\n",
                    parameters(params)
                );
                if let Some((kind, params)) = disposition {
                    let params = parameters(params);
                    write!(headers, "Content-Disposition: {kind}{params}\r\n").unwrap();
                }

                if let Some(id) = id {
                    write!(headers, "Content-ID: <{id}>\r\n").unwrap();
                }

                headers
            }
            Self::Multipart { subtype, .. } => format!(
                "Content-Type: multipart/{subtype};\r\n boundary=\"{}\"\r\n",
                boundary(id)
            ),
        }
    }

    /// Appends this part's content after its headers; `id` keeps the
    /// boundaries of nested multiparts apart.
    fn content(&self, out: &mut Vec<u8>, id: &str) {
        match self {
            Self::Part { content, .. } => out.extend_from_slice(content),
            Self::Multipart { parts, .. } => {
                let boundary = boundary(id);
                out.extend_from_slice(b"This is a multi-part message in MIME format.\r\n");
                for (index, part) in (1..).zip(parts) {
                    let id = format!("{id}.{index}");
                    out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                    out.extend_from_slice(part.headers(&id).as_bytes());
                    out.extend_from_slice(b"\r\n");
                    part.content(out, &id);
                    out.extend_from_slice(b"\r\n");
                }

                out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
            }
        }
    }

    /// The part at `path` and the id its boundary is named by; a Message
    /// that is not multipart is its own part 1.
    fn node(&self, path: &[NonZeroU32]) -> Option<(&Self, String)> {
        let mut node = (self, "1".to_owned());
        if let (Self::Part { .. }, [one]) = (self, path) {
            return (one.get() == 1).then_some(node);
        }

        for number in path {
            let Self::Multipart { parts, .. } = node.0 else {
                return None;
            };

            let part = parts.get(usize::try_from(number.get() - 1).unwrap())?;
            node = (part, format!("{}.{number}", node.1));
        }

        Some(node)
    }

    /// The octets of part `path`, as in `BODY[1.2]`.
    pub fn part(&self, path: &[NonZeroU32]) -> Option<Vec<u8>> {
        let (node, id) = self.node(path)?;
        let mut content = Vec::new();
        node.content(&mut content, &id);
        Some(content)
    }

    /// The headers of part `path`, as in `BODY[1.2.MIME]`, the blank line
    /// after them included.
    pub fn mime(&self, path: &[NonZeroU32]) -> Option<String> {
        let (node, id) = self.node(path)?;
        Some(format!("{}\r\n", node.headers(&id)))
    }
}

fn boundary(id: &str) -> String {
    format!("=_boundary_{id}")
}

/// `params` as a header writes them, each after `; `; an RFC 2231 value
/// goes unquoted.
fn parameters(params: &[(String, String)]) -> String {
    params
        .iter()
        .map(|(key, value)| {
            if key.ends_with('*') {
                format!("; {key}={value}")
            } else {
                format!("; {key}=\"{value}\"")
            }
        })
        .collect()
}
