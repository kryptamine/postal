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
    /// A name that only repeats the address is none.
    pub fn new(address: &str, name: Option<String>, written: bool) -> Self {
        let name = name.filter(|name| {
            let name = name.trim().trim_matches(['"', '\'', '<', '>']);
            !name.is_empty() && !name.eq_ignore_ascii_case(address)
        });

        Self {
            address: address.to_lowercase(),
            name,
            written,
        }
    }
}
