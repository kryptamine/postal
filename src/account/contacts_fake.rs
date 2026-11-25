//! Test seam: a test cannot fill the system's Contacts nor answer its
//! permission prompt, so the OS boundary is faked here, not a mail server.

use std::path::PathBuf;

use crate::account::contacts::{Person, Source, Unavailable};

/// `POSTAL_CONTACTS_FAKE` names a JSON file of `{"name", "emails"}`
/// people, or is `denied` for a refused permission.
pub fn from_env() -> Option<Box<dyn Source>> {
    let value = std::env::var_os("POSTAL_CONTACTS_FAKE")?;
    Some(Box::new(Fake(PathBuf::from(value))))
}

struct Fake(PathBuf);

impl Source for Fake {
    fn people(&self) -> Result<Vec<Person>, Unavailable> {
        if self.0.as_os_str() == "denied" {
            return Err(Unavailable::Denied);
        }

        let text = std::fs::read_to_string(&self.0)
            .map_err(|error| Unavailable::Failed(error.to_string()))?;
        serde_json::from_str(&text).map_err(|error| Unavailable::Failed(error.to_string()))
    }
}
