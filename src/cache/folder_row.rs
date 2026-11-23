use rusqlite::OptionalExtension;

use super::{Error, Reader};
use crate::domain::folder::Role;

/// One Folder as the last sync left it.
pub struct FolderRow {
    /// The Account's address, lowercased.
    pub account: String,
    pub name: String,
    pub role: Option<Role>,
    /// `None` until a resync or a `STATUS` counted it.
    pub unread: Option<u32>,
}

impl Reader {
    /// Every Account's Folders, by Account, then name.
    pub fn folders(&self) -> Result<Vec<FolderRow>, Error> {
        let mut statement = self
            .connection
            .prepare("SELECT account, name, role, unread FROM mailbox ORDER BY account, name")?;
        let rows = statement
            .query_map([], |row| {
                let role: Option<String> = row.get(2)?;
                Ok(FolderRow {
                    account: row.get(0)?,
                    name: row.get(1)?,
                    role: role.as_deref().and_then(Role::from_label),
                    unread: row.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// The name of the Folder of `account` that `folder` names, by its name
    /// or its role's label in any case, an exact name first; `None` for neither.
    pub fn folder_named(&self, account: &str, folder: &str) -> Result<Option<String>, Error> {
        Ok(self
            .connection
            .query_row(
                "SELECT name FROM mailbox
                 WHERE account = ?1 AND (name = ?2 COLLATE NOCASE OR role = ?2 COLLATE NOCASE)
                 ORDER BY name != ?2 LIMIT 1",
                [account, folder],
                |row| row.get(0),
            )
            .optional()?)
    }
}
