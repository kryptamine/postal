use std::{
    io,
    path::{Path, PathBuf},
};

use toml_edit::{
    ArrayOfTables, DocumentMut, InlineTable, Item, RawString, Table, TableLike, Value, value,
};

use crate::{
    account::atomic_file,
    account::config::{Error, IconChoice, Imap, NotifyChoice, SecretStore, Smtp, Terminal},
    account::provider::Provider,
};

/// `config.toml` as it is written, changed key by key: the comments and
/// the layout around a change stay as the user left them.
pub struct ConfigFile {
    path: PathBuf,
    document: DocumentMut,
}

impl ConfigFile {
    /// An empty file while there is none.
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(source) if source.kind() == io::ErrorKind::NotFound => String::new(),
            Err(source) => {
                return Err(Error::Read {
                    path: path.to_owned(),
                    source,
                });
            }
        };

        let document = text.parse::<DocumentMut>().map_err(|error| {
            let at = error.span().map_or(0, |span| span.start);
            Error::invalid(path, &text, at, error.message())
        })?;
        Ok(Self {
            path: path.to_owned(),
            document,
        })
    }

    pub fn save(&self) -> io::Result<()> {
        atomic_file::write(&self.path, self.document.to_string().as_bytes(), 0o644)
    }

    /// Adds the Account, or replaces its Provider. The first goes at the top
    /// level, whose tables are every Account's; the others, and any with
    /// servers of its own, in an `[[account]]`.
    pub fn set_account(&mut self, email: &str, provider: Provider) {
        let root = self.document.as_table_mut();
        tabled(root);
        if names(root, email) && provider == Provider::Other {
            let mut account = Table::new();
            for key in ["email", "signature", "signature-file"] {
                if let Some(item) = root.remove(key) {
                    account.insert(key, item);
                }
            }

            root.remove("provider");
            set_provider(&mut account, provider);
            account.decor_mut().set_prefix("\n");
            if let Some(accounts) = accounts_mut(root) {
                accounts.push(account);
            }
        } else if names(root, email) {
            set_provider(root, provider);
        } else if let Some(account) = account_mut(root, email) {
            set_provider(account, provider);
        } else if provider == Provider::Other
            || root.contains_key("email")
            || root.contains_key("account")
        {
            let mut account = Table::new();
            account.insert("email", value(email));
            set_provider(&mut account, provider);
            account.decor_mut().set_prefix("\n");
            if let Some(accounts) = accounts_mut(root) {
                accounts.push(account);
            }
        } else {
            if !root.iter().any(|(_, item)| item.is_value()) {
                space_tables(root);
            }

            root.insert("email", value(email));
            set_provider(root, provider);
            // The address leads the file, ahead of what was written before it.
            root.sort_values_by(|a, _, b, _| rank(a.get()).cmp(&rank(b.get())));
        }
    }

    /// Writes the servers into the `[[account]]` of `email`, each as an inline
    /// table with every key but an unset username; nothing without one.
    pub fn set_servers(&mut self, email: &str, imap: &Imap, smtp: &Smtp) {
        let root = self.document.as_table_mut();
        tabled(root);
        let Some(account) = account_mut(root, email) else {
            return;
        };

        let mut table = InlineTable::new();
        table.insert("host", imap.host.as_str().into());
        table.insert("port", i64::from(imap.port).into());
        table.insert("tls", imap.tls.into());
        if let Some(username) = &imap.username {
            table.insert("username", username.as_str().into());
        }

        set(account, "imap", Value::InlineTable(table));
        let mut table = InlineTable::new();
        table.insert("host", smtp.host.as_str().into());
        table.insert("port", i64::from(smtp.port).into());
        table.insert("tls", smtp.tls.into());
        table.insert("starttls", smtp.starttls.into());
        if let Some(username) = &smtp.username {
            table.insert("username", username.as_str().into());
        }

        set(account, "smtp", Value::InlineTable(table));
    }

    pub fn remove_account(&mut self, email: &str) {
        let root = self.document.as_table_mut();
        tabled(root);
        if names(root, email) {
            // Its signature would sign for the next Account written here.
            for key in ["email", "provider", "signature", "signature-file"] {
                root.remove(key);
            }

            return;
        }

        let Some(accounts) = accounts_mut(root) else {
            return;
        };

        let at = accounts.iter().position(|account| names(account, email));
        if let Some(at) = at {
            accounts.remove(at);
        }

        if accounts.is_empty() {
            root.remove("account");
        }
    }

    /// The display name every Account shares; `None` writes none.
    pub fn set_name(&mut self, name: Option<&str>) {
        let root = self.document.as_table_mut();
        match name {
            Some(name) => set(root, "name", name.into()),
            None => drop(root.remove("name")),
        }
    }

    /// The signature of the Account `email` as text, where the Account is
    /// written; `None` writes none.
    pub fn set_signature(&mut self, email: &str, signature: Option<&str>) {
        let root = self.document.as_table_mut();
        tabled(root);
        let account = if names(root, email) {
            Some(root)
        } else {
            account_mut(root, email)
        };

        match (account, signature) {
            (Some(account), Some(signature)) => set(account, "signature", signature.into()),
            (Some(account), None) => drop(account.remove("signature")),
            (None, _) => {}
        }
    }

    pub fn set_icons(&mut self, icons: IconChoice) {
        let root = self.document.as_table_mut();
        if icons == IconChoice::default() {
            root.remove("icons");
        } else {
            set(root, "icons", icons.key().into());
        }
    }

    pub fn set_notify(&mut self, notify: NotifyChoice) {
        let root = self.document.as_table_mut();
        if notify == NotifyChoice::default() {
            root.remove("notify");
        } else {
            set(root, "notify", notify.key().into());
        }
    }

    pub fn set_terminal(&mut self, terminal: &Terminal) {
        let root = self.document.as_table_mut();
        match terminal {
            _ if *terminal == Terminal::default() => drop(root.remove("terminal")),
            Terminal::App(app) => set(root, "terminal", app.key().into()),
            Terminal::Command(command) => {
                let mut table = InlineTable::new();
                table.insert("command", command.as_str().into());
                set(root, "terminal", Value::InlineTable(table));
            }
        }
    }

    pub fn set_secrets(&mut self, store: SecretStore) {
        let root = self.document.as_table_mut();
        if store == SecretStore::default() {
            let secrets = root.get_mut("secrets").and_then(Item::as_table_like_mut);
            let emptied = secrets.is_some_and(|secrets| {
                secrets.remove("store");
                secrets.is_empty()
            });

            if emptied {
                root.remove("secrets");
            }

            return;
        }

        let mut table = Table::new();
        table.decor_mut().set_prefix("\n");
        let secrets = root.entry("secrets").or_insert(Item::Table(table));
        if let Some(secrets) = secrets.as_table_like_mut() {
            set(secrets, "store", store.key().into());
        }
    }
}

/// A blank line ahead of the first table, for the keys about to lead it.
fn space_tables(root: &mut Table) {
    let tables = root.iter_mut().filter_map(|(_, item)| item.as_table_mut());
    let Some(first) = tables.min_by_key(|table| table.position()) else {
        return;
    };

    let prefix = first.decor().prefix().and_then(RawString::as_str);
    let prefix = prefix.unwrap_or_default();
    if !prefix.starts_with('\n') {
        let spaced = format!("\n{prefix}");
        first.decor_mut().set_prefix(spaced);
    }
}

/// `email` and `provider` ahead of every other top-level key.
fn rank(key: &str) -> u8 {
    match key {
        "email" => 0,
        "provider" => 1,
        _ => 2,
    }
}

fn names(account: &Table, email: &str) -> bool {
    let written = account.get("email").and_then(Item::as_str);
    written.is_some_and(|written| written.eq_ignore_ascii_case(email))
}

/// Accounts written as an inline array become `[[account]]` tables,
/// which is all a new one can be.
fn tabled(root: &mut Table) {
    let Some(item) = root.get_mut("account") else {
        return;
    };

    if item.is_array_of_tables() {
        return;
    }

    *item = match std::mem::take(item).into_array_of_tables() {
        Ok(tables) => Item::ArrayOfTables(tables),
        Err(other) => other,
    };

    // The space before `=` would end up inside the `[[account]]` header.
    if let Some(mut key) = root.key_mut("account") {
        key.leaf_decor_mut().set_suffix("");
    }
}

/// The `[[account]]` tables, made when there are none.
fn accounts_mut(root: &mut Table) -> Option<&mut ArrayOfTables> {
    let empty = Item::ArrayOfTables(ArrayOfTables::new());
    root.entry("account")
        .or_insert(empty)
        .as_array_of_tables_mut()
}

fn account_mut<'a>(root: &'a mut Table, email: &str) -> Option<&'a mut Table> {
    let mut accounts = root
        .get_mut("account")?
        .as_array_of_tables_mut()?
        .iter_mut();
    accounts.find(|account| names(account, email))
}

/// The default Provider is left out.
fn set_provider(account: &mut Table, provider: Provider) {
    if provider == Provider::default() {
        account.remove("provider");
    } else {
        set(account, "provider", provider.key().into());
    }
}

/// Sets `key`, keeping the comment beside a value it replaces.
fn set(table: &mut dyn TableLike, key: &str, mut new: Value) {
    match table.get_mut(key).and_then(Item::as_value_mut) {
        Some(old) => {
            *new.decor_mut() = old.decor().clone();
            *old = new;
        }
        None => drop(table.insert(key, Item::Value(new))),
    }
}
