use crate::domain::folder::Folder;

/// The server's name of `account`'s Folder that `wanted` names: by that
/// name first, else as people name it.
pub fn named<'a>(folders: &'a [Folder<String>], account: &str, wanted: &str) -> Option<&'a str> {
    let mine = || folders.iter().filter(|f| f.account == *account);
    mine()
        .find(|f| f.name.as_str() == wanted)
        .or_else(|| mine().find(|f| f.answers_to(wanted)))
        .map(|f| f.name.as_str())
}
