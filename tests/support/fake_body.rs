//! A Message body as the fake IMAP server serves it: its `BODYSTRUCTURE`,
//! and the octets of one part by part number.

use std::{fmt::Write, num::NonZeroU32};

use imap_next::imap_types::{
    body::{
        BasicFields, Body, BodyStructure, Disposition, SinglePartExtensionData, SpecificFields,
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
        let string = |s: &str| IString::try_from(s.to_owned()).unwrap();
        match self {
            Self::Part {
                media_type,
                subtype,
                charset,
                encoding,
                content,
                disposition,
                params,
                id,
            } => {
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
                        number_of_lines: u32::try_from(content.split(|&b| b == b'\n').count())
                            .unwrap(),
                    }
                } else {
                    SpecificFields::Basic {
                        r#type: string(media_type),
                        subtype: string(subtype),
                    }
                };

                let extension_data =
                    disposition
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
            Self::Multipart { subtype, parts } => BodyStructure::Multi {
                bodies: Vec1::try_from(parts.iter().map(Self::structure).collect::<Vec<_>>())
                    .unwrap(),
                subtype: string(subtype),
                extension_data: None,
            },
        }
    }

    /// The Message as `BODY[]` serves it: `headers`, each ending in CRLF,
    /// then this body in MIME.
    pub fn message(&self, headers: &str) -> Vec<u8> {
        let mut message = format!("{headers}MIME-Version: 1.0\r\n").into_bytes();
        self.entity(&mut message, "1");
        message
    }

    /// Appends this part's own headers, a blank line, and its content;
    /// `id` keeps the boundaries of nested multiparts apart.
    fn entity(&self, out: &mut Vec<u8>, id: &str) {
        match self {
            Self::Part {
                media_type,
                subtype,
                charset,
                encoding,
                content,
                disposition,
                params,
                id,
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

                out.extend_from_slice(headers.as_bytes());
                out.extend_from_slice(b"\r\n");
                out.extend_from_slice(content);
            }
            Self::Multipart { subtype, parts } => {
                let boundary = format!("=_boundary_{id}");
                out.extend_from_slice(
                    format!(
                        "Content-Type: multipart/{subtype};\r\n boundary=\"{boundary}\"\r\n\r\n"
                    )
                    .as_bytes(),
                );
                out.extend_from_slice(b"This is a multi-part message in MIME format.\r\n");
                for (index, part) in (1..).zip(parts) {
                    out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                    part.entity(out, &format!("{id}.{index}"));
                    out.extend_from_slice(b"\r\n");
                }

                out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
            }
        }
    }

    /// The octets of part `path`, as in `BODY[1.2]`.
    pub fn part(&self, path: &[NonZeroU32]) -> Option<&[u8]> {
        match (self, path) {
            (Self::Part { content, .. }, [one]) if one.get() == 1 => Some(content),
            (Self::Multipart { parts, .. }, [first, rest @ ..]) => {
                let part = parts.get(usize::try_from(first.get() - 1).unwrap())?;
                match (part, rest) {
                    (Self::Part { content, .. }, []) => Some(content),
                    (Self::Multipart { .. }, [_, ..]) => part.part(rest),
                    _ => None,
                }
            }
            _ => None,
        }
    }
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
