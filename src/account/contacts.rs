use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::account::config::ContactsChoice;

/// Whether this system has Contacts to read; elsewhere `contacts` is ignored.
pub const READS_CONTACTS: bool = cfg!(target_os = "macos");

/// One email address of a person in the system's Contacts.
pub struct Contact {
    /// Lowercased.
    pub address: String,
    /// The name the user gave the person; `None` when they gave none.
    pub name: Option<String>,
}

/// A person as a source of Contacts holds them.
#[derive(Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    pub emails: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[cfg_attr(
    not(target_os = "macos"),
    expect(dead_code, reason = "only the macOS sources fail")
)]
pub enum Unavailable {
    #[error("access to Contacts is denied: Correspondents come from mail alone")]
    Denied,
    #[error("could not read Contacts: {0}")]
    Failed(String),
}

/// Where the people come from: the system's address book, or a test's.
pub trait Source {
    fn people(&self) -> Result<Vec<Person>, Unavailable>;
}

/// Every address in Contacts once, by name then address, those without a
/// name last; none unless `choice` asks for them. Never copied anywhere:
/// read anew each time (ADR 0035).
pub fn read(choice: ContactsChoice) -> Result<Vec<Contact>, Unavailable> {
    match choice {
        ContactsChoice::None => Ok(Vec::new()),
        ContactsChoice::Macos => Ok(flatten(source().people()?)),
    }
}

/// `read`, with what kept it from reading logged; none then.
pub fn read_or_log(choice: ContactsChoice) -> Vec<Contact> {
    read(choice).unwrap_or_else(|error| {
        tracing::warn!("{error}");
        Vec::new()
    })
}

#[cfg(target_os = "macos")]
fn source() -> Box<dyn Source> {
    Box::new(crate::account::contacts_process::ContactsProcess)
}

#[cfg(not(target_os = "macos"))]
fn source() -> Box<dyn Source> {
    tracing::warn!("`contacts = \"macos\"` in config.toml is ignored off macOS");
    Box::new(Nobody)
}

#[cfg(not(target_os = "macos"))]
struct Nobody;

#[cfg(not(target_os = "macos"))]
impl Source for Nobody {
    fn people(&self) -> Result<Vec<Person>, Unavailable> {
        Ok(Vec::new())
    }
}

fn flatten(people: Vec<Person>) -> Vec<Contact> {
    let mut contacts: Vec<Contact> = people
        .into_iter()
        .flat_map(|person| {
            let name = Some(person.name.trim().to_owned()).filter(|name| !name.is_empty());
            let emails = person.emails.into_iter();
            emails.map(move |email| Contact {
                address: email.trim().to_lowercase(),
                name: name.clone(),
            })
        })
        .filter(|contact| contact.address.contains('@'))
        .collect();

    contacts.sort_by(|a, b| {
        let by_name = (a.name.is_none(), &a.name).cmp(&(b.name.is_none(), &b.name));
        by_name.then_with(|| a.address.cmp(&b.address))
    });

    let mut seen = HashSet::new();
    contacts.retain(|contact| seen.insert(contact.address.clone()));
    contacts
}
