/// The addresses one Message names, as Correspondents are learnt from it.
pub struct Correspondence {
    /// Its `Message-ID`, else a key of its own: a copy in another Folder,
    /// or the same Message sent and then fetched from Sent, counts once.
    pub message: String,
    pub people: Vec<Person>,
}

pub struct Person {
    /// Lowercased.
    pub address: String,
    pub name: Option<String>,
    /// Whether the Account wrote to it, rather than had mail from or with it.
    pub written: bool,
}

impl Person {
    pub fn new(address: &str, name: Option<String>, written: bool) -> Self {
        Self {
            address: address.to_lowercase(),
            name: name.filter(|name| !name.trim().is_empty()),
            written,
        }
    }
}
