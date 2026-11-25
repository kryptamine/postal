use std::{cell::RefCell, ptr::NonNull};

use block2::StackBlock;
use objc2::{
    AnyThread,
    rc::Retained,
    runtime::{Bool, ProtocolObject},
};
use objc2_contacts::{
    CNAuthorizationStatus, CNContact, CNContactEmailAddressesKey, CNContactFamilyNameKey,
    CNContactFetchRequest, CNContactGivenNameKey, CNContactStore, CNEntityType, CNKeyDescriptor,
};
use objc2_foundation::{NSArray, NSError, NSString};

use crate::account::contacts::{Person, Source, Unavailable};

/// The system's Contacts, through `CNContactStore`.
pub struct AddressBook;

impl Source for AddressBook {
    /// Off the main thread the store waits while macOS asks the user for
    /// access, so the first read blocks until they answer.
    fn people(&self) -> Result<Vec<Person>, Unavailable> {
        if refused() {
            return Err(Unavailable::Denied);
        }

        // SAFETY: the keys are immutable statics the framework exports.
        let keys: [&NSString; 3] = unsafe {
            [
                CNContactGivenNameKey,
                CNContactFamilyNameKey,
                CNContactEmailAddressesKey,
            ]
        };

        let keys = keys.map(ProtocolObject::<dyn CNKeyDescriptor>::from_ref);
        let keys = NSArray::from_slice(&keys);
        // SAFETY: the keys are those the framework names for a contact.
        let request = unsafe {
            CNContactFetchRequest::initWithKeysToFetch(CNContactFetchRequest::alloc(), &keys)
        };

        // SAFETY: a store takes no arguments to make.
        let store = unsafe { CNContactStore::new() };

        let people = RefCell::new(Vec::new());
        let each = StackBlock::new(|contact: NonNull<CNContact>, _stop: NonNull<Bool>| {
            // SAFETY: the store hands a live contact for the call's length.
            let contact = unsafe { contact.as_ref() };
            people.borrow_mut().push(person(contact));
        });

        let mut error: Option<Retained<NSError>> = None;
        // SAFETY: the request fetches only the keys `person` reads, and the
        // block outlives the call, which is synchronous.
        let done = unsafe {
            store.enumerateContactsWithFetchRequest_error_usingBlock(
                &request,
                Some(&mut error),
                &each,
            )
        };

        if done {
            return Ok(people.into_inner());
        }

        if refused() {
            return Err(Unavailable::Denied);
        }

        let reason = error.map_or_else(
            || "unknown error".to_owned(),
            |error| error.localizedDescription().to_string(),
        );
        Err(Unavailable::Failed(reason))
    }
}

fn refused() -> bool {
    // SAFETY: a class method with no state of ours.
    let status =
        unsafe { CNContactStore::authorizationStatusForEntityType(CNEntityType::Contacts) };
    status == CNAuthorizationStatus::Denied || status == CNAuthorizationStatus::Restricted
}

fn person(contact: &CNContact) -> Person {
    // SAFETY: the request fetched these keys, so reading them cannot throw.
    let (given, family, emails) = unsafe {
        (
            contact.givenName(),
            contact.familyName(),
            contact.emailAddresses(),
        )
    };

    let name = format!("{given} {family}");
    let emails = emails
        .iter()
        // SAFETY: a labelled email address holds a string.
        .map(|labelled| unsafe { labelled.value() }.to_string())
        .collect();
    Person { name, emails }
}
