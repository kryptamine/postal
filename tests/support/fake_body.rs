//! A Message body as the fake IMAP server serves it: its `BODYSTRUCTURE`,
//! and the octets of one part by part number.

use std::num::NonZeroU32;

use imap_next::imap_types::{
    body::{
        BasicFields, Body, BodyStructure, Disposition, SinglePartExtensionData, SpecificFields,
    },
    core::{IString, NString, Vec1},
};

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
        attachment: bool,
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
            attachment: false,
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

    pub const fn attached(mut self) -> Self {
        if let Self::Part { attachment, .. } = &mut self {
            *attachment = true;
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
            attachment: true,
        }
    }

    pub const fn multipart(subtype: &'static str, parts: Vec<Self>) -> Self {
        Self::Multipart { subtype, parts }
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
                attachment,
            } => {
                let basic = BasicFields {
                    parameter_list: charset
                        .iter()
                        .map(|c| (string("charset"), string(c)))
                        .collect(),
                    id: NString(None),
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

                let extension_data = attachment.then(|| SinglePartExtensionData {
                    md5: NString(None),
                    tail: Some(Disposition {
                        disposition: Some((string("attachment"), vec![])),
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
