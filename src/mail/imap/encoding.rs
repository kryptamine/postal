use async_imap::imap_proto::ContentEncoding;

use crate::domain::transfer_encoding::TransferEncoding;

pub fn of(sent: &ContentEncoding<'_>) -> TransferEncoding {
    match sent {
        ContentEncoding::QuotedPrintable => TransferEncoding::QuotedPrintable,
        ContentEncoding::Base64 => TransferEncoding::Base64,
        ContentEncoding::Other(label) => TransferEncoding::from_label(label),
        ContentEncoding::SevenBit | ContentEncoding::EightBit | ContentEncoding::Binary => {
            TransferEncoding::Identity
        }
    }
}
