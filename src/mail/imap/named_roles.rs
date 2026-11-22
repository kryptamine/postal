use crate::domain::folder::{Folder, Role};

/// The names a server without SPECIAL-USE (RFC 6154) gives a role's
/// Folder: iCloud's first, then the common ones.
const NAMES: [(Role, &[&str]); 5] = [
    (Role::Sent, &["Sent Messages", "Sent"]),
    (Role::Drafts, &["Drafts"]),
    (Role::Archive, &["Archive"]),
    (Role::Spam, &["Junk", "Spam"]),
    (Role::Trash, &["Deleted Messages", "Trash"]),
];

/// Gives each role no Folder has by its special use to the Folder with one
/// of its names; a role named by special use is never given twice.
pub fn fill(folders: &mut [Folder<()>]) {
    for (role, names) in NAMES {
        // Where All Mail is, as on Gmail, archiving goes there and a Folder
        // named Archive is a label like any other.
        let taken = |r| folders.iter().any(|f| f.role == Some(r));
        if taken(role) || (role == Role::Archive && taken(Role::All)) {
            continue;
        }

        let named = names.iter().find_map(|name| {
            folders
                .iter()
                .position(|f| f.role.is_none() && f.name.as_str().eq_ignore_ascii_case(name))
        });

        if let Some(folder) = named.and_then(|at| folders.get_mut(at)) {
            folder.role = Some(role);
        }
    }
}
