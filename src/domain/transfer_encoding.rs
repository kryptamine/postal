use base64::{
    Engine,
    alphabet::STANDARD,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};

/// A part's `Content-Transfer-Encoding`, reduced to what decoding needs.
#[derive(Clone, Copy)]
pub enum TransferEncoding {
    /// `7bit`, `8bit`, `binary`, and any encoding Postal does not know.
    Identity,
    QuotedPrintable,
    Base64,
}

/// Padding is dropped before decoding, so none is expected.
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::RequireNone)
        .with_decode_allow_trailing_bits(true),
);

impl TransferEncoding {
    pub const fn from_label(label: &str) -> Self {
        if label.eq_ignore_ascii_case("quoted-printable") {
            Self::QuotedPrintable
        } else if label.eq_ignore_ascii_case("base64") {
            Self::Base64
        } else {
            Self::Identity
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::QuotedPrintable => "quoted-printable",
            Self::Base64 => "base64",
        }
    }

    /// Decodes a prefix of a part: an escape or a base64 group cut off at
    /// the end is dropped rather than failing the whole prefix.
    pub fn decode(self, bytes: &[u8]) -> Vec<u8> {
        match self {
            Self::Identity => bytes.to_vec(),
            Self::QuotedPrintable => {
                let cut = bytes.len().saturating_sub(2);
                let end = bytes[cut..]
                    .iter()
                    .position(|&b| b == b'=')
                    .map_or(bytes.len(), |at| cut + at);
                quoted_printable::decode(&bytes[..end], quoted_printable::ParseMode::Robust)
                    .unwrap_or_default()
            }
            Self::Base64 => {
                let mut digits: Vec<u8> = bytes
                    .iter()
                    .copied()
                    .filter(|b| b.is_ascii_alphanumeric() || *b == b'+' || *b == b'/')
                    .collect();
                // One digit alone carries no whole byte.
                if digits.len() % 4 == 1 {
                    digits.pop();
                }

                BASE64.decode(digits).unwrap_or_default()
            }
        }
    }
}
