use chrono::{DateTime, Utc};

/// The addresses one Message names, as Correspondents are learnt from it.
pub struct Correspondence {
    /// Its `Message-ID` as `key` gives it, so a copy in another Folder, or
    /// the same Message sent and then fetched from Sent, counts once.
    pub message: String,
    pub at: DateTime<Utc>,
    pub from: Vec<Person>,
    /// `To`, then `Cc`.
    pub to: Vec<Person>,
    /// Whether the Account sent it, so it wrote to those in `to`.
    pub sent: bool,
}

pub struct Person {
    /// Lowercased.
    pub address: String,
    pub name: Option<String>,
}

impl Person {
    /// A name that only repeats the address is none.
    pub fn new(address: &str, name: Option<String>) -> Self {
        let name = name.filter(|name| {
            let name = name.trim().trim_matches(['"', '\'', '<', '>']);
            !name.is_empty() && !name.eq_ignore_ascii_case(address)
        });

        Self {
            address: address.to_lowercase(),
            name,
        }
    }
}

/// The key a `Message-ID` is known by, however a header or an envelope
/// wrote it: without its angle brackets, lowercased.
pub fn key(message_id: &str) -> String {
    message_id
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_lowercase()
}
