use super::{Error, Reader, folders};
use crate::domain::folder::{Folder, Role};

impl Reader {
    /// Every Account's Folders as the last sync left them, each by its
    /// Account's address, lowercased.
    pub fn folders(&self) -> Result<Vec<Folder<String>>, Error> {
        let mut statement = self
            .connection
            .prepare("SELECT account, name, role, unread, messages FROM mailbox")?;
        let rows = statement
            .query_map([], |row| {
                let role: Option<String> = row.get(2)?;
                let account: String = row.get(0)?;
                let name: String = row.get(1)?;
                let mut folder =
                    Folder::new(account, &name, role.as_deref().and_then(Role::from_label));
                folder.counts = folders::counts(row.get(3)?, row.get(4)?);
                Ok(folder)
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}
