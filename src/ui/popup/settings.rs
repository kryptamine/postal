use std::{
    error::Error,
    path::{Path, PathBuf},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    account::config::{self, IconChoice, NotifyChoice, SecretStore},
    account::config_file::ConfigFile,
    account::partial_config::PartialConfig,
    account::provider::Provider,
    account::sign_in::smtp_key,
    ui::onboarding::field::{self, Edit},
    ui::popup::settings_section::Section,
};

pub const ICONS: [IconChoice; 3] = [IconChoice::Auto, IconChoice::Nerd, IconChoice::None];
pub const NOTIFY: [NotifyChoice; 3] = [
    NotifyChoice::Unfocused,
    NotifyChoice::Always,
    NotifyChoice::Never,
];
pub const SECRETS: [SecretStore; 2] = [SecretStore::Keychain, SecretStore::File];

/// One row of a Settings tab the selection can rest on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// The Account at this place in `config.toml`.
    Account(usize),
    /// The signature of the Account at this place.
    Signature(usize),
    Add,
    /// The value at this place in the tab's list of them.
    Choice(usize),
    Name,
}

/// What a key in Settings leaves to the event loop.
pub enum Step {
    Stay,
    /// Settings closed; `reload` when a setting the running Accounts read
    /// at start changed.
    Close {
        reload: bool,
    },
    Help,
    AddAccount,
    Unlink(String),
    /// The editor on the signature of the Account at this address.
    Signature(String),
    Icons(IconChoice),
    Notify(NotifyChoice),
    Quit,
}

/// The settings of `config.toml`, each change written at once.
pub struct Settings {
    dir: PathBuf,
    pub accounts: Vec<(String, Provider)>,
    /// The first line of each Account's signature, where it reads.
    pub signatures: Vec<Option<String>>,
    pub icons: IconChoice,
    pub notify: NotifyChoice,
    pub name: Option<String>,
    pub secrets: SecretStore,
    section: Section,
    /// Index into the tab's rows.
    selected: usize,
    /// The Name being typed, while it is.
    pub editing: Option<String>,
    /// The Account `d` asked to unlink, until `y` or `n`.
    pub unlinking: Option<usize>,
    /// Why the last change was not made.
    pub failure: Option<String>,
    reload: bool,
}

impl Settings {
    /// Settings as `config.toml` in `dir` has them now.
    pub fn open(dir: &Path) -> Result<Self, config::Error> {
        let config = PartialConfig::load(&dir.join("config.toml"))?;
        Ok(Self {
            dir: dir.to_owned(),
            signatures: signatures(&config),
            accounts: config
                .accounts()
                .map(|(email, provider)| (email.to_owned(), provider))
                .collect(),
            icons: config.icons,
            notify: config.notify,
            name: config.name,
            secrets: config.secrets,
            section: Section::Accounts,
            selected: 0,
            editing: None,
            unlinking: None,
            failure: None,
            reload: false,
        })
    }

    pub const fn section(&self) -> Section {
        self.section
    }

    /// The rows of the tab shown.
    pub fn rows(&self) -> impl Iterator<Item = Row> {
        self.rows_of(self.section)
    }

    /// The rows of `section`: its Accounts, or its values.
    pub fn rows_of(&self, section: Section) -> impl Iterator<Item = Row> {
        let accounts = if section == Section::Accounts {
            0..self.accounts.len()
        } else {
            0..0
        };
        let accounts =
            accounts.flat_map(|account| [Row::Account(account), Row::Signature(account)]);
        let rest = match section {
            Section::Icons => 0..ICONS.len(),
            Section::Notify => 0..NOTIFY.len(),
            Section::Secrets => 0..SECRETS.len(),
            Section::Accounts | Section::Name => 0..0,
        };
        let last = match section {
            Section::Accounts => Some(Row::Add),
            Section::Name => Some(Row::Name),
            _ => None,
        };
        accounts.chain(rest.map(Row::Choice)).chain(last)
    }

    pub fn selected(&self) -> Row {
        self.rows().nth(self.selected).unwrap_or(Row::Add)
    }

    /// The place of the value in effect among the tab's rows.
    pub fn current(&self) -> Option<usize> {
        match self.section {
            Section::Icons => ICONS.iter().position(|&c| c == self.icons),
            Section::Notify => NOTIFY.iter().position(|&c| c == self.notify),
            Section::Secrets => SECRETS.iter().position(|&c| c == self.secrets),
            Section::Accounts | Section::Name => None,
        }
    }

    /// Shows the tab `step` places on, its value in effect selected.
    fn switch(&mut self, step: isize) {
        self.section = self.section.moved(step);
        self.selected = self.current().unwrap_or(0);
        self.failure = None;
    }

    pub fn key(&mut self, key: KeyEvent) -> Step {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            return Step::Quit;
        }

        if let Some(typed) = &mut self.editing {
            match field::edit(typed, key) {
                Edit::Submit => {
                    let name = Some(typed.trim().to_owned()).filter(|name| !name.is_empty());
                    self.editing = None;
                    self.change(|settings| settings.rename(name));
                }
                Edit::Back => self.editing = None,
                Edit::Cancel => return Step::Quit,
                Edit::Changed | Edit::Ignored => {}
            }

            return Step::Stay;
        }

        if let Some(account) = self.unlinking.take() {
            return match (key.code, self.accounts.get(account)) {
                (KeyCode::Char('y' | 'Y'), Some((email, _))) => Step::Unlink(email.clone()),
                _ => Step::Stay,
            };
        }

        if ctrl {
            return Step::Stay;
        }

        let last = self.rows().count().saturating_sub(1);
        match key.code {
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => self.switch(1),
            KeyCode::BackTab | KeyCode::Char('h') | KeyCode::Left => self.switch(-1),
            KeyCode::Char('j') | KeyCode::Down => self.selected = (self.selected + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('d') | KeyCode::Backspace => {
                if let Row::Account(account) = self.selected() {
                    self.failure = None;
                    self.unlinking = Some(account);
                }
            }
            KeyCode::Enter => return self.act(),
            KeyCode::Char('?') => return Step::Help,
            KeyCode::Char('q' | ',') | KeyCode::Esc => {
                return Step::Close {
                    reload: self.reload,
                };
            }
            _ => {}
        }

        Step::Stay
    }

    fn act(&mut self) -> Step {
        match self.selected() {
            Row::Account(_) => {}
            Row::Signature(account) => {
                if let Some((email, _)) = self.accounts.get(account) {
                    self.failure = None;
                    return Step::Signature(email.clone());
                }
            }
            Row::Add => return Step::AddAccount,
            Row::Name => self.editing = Some(self.name.clone().unwrap_or_default()),
            Row::Choice(at) => return self.choose(at),
        }

        Step::Stay
    }

    /// Puts the value at `at` of the tab shown in effect.
    fn choose(&mut self, at: usize) -> Step {
        match self.section {
            Section::Icons => {
                let icons = ICONS[at];
                if icons != self.icons
                    && self.change(|settings| settings.write(|config| config.set_icons(icons)))
                {
                    self.icons = icons;
                    return Step::Icons(icons);
                }
            }
            Section::Notify => {
                let notify = NOTIFY[at];
                if notify != self.notify
                    && self.change(|settings| settings.write(|config| config.set_notify(notify)))
                {
                    self.notify = notify;
                    return Step::Notify(notify);
                }
            }
            Section::Secrets => {
                let store = SECRETS[at];
                if store != self.secrets {
                    self.change(|settings| settings.switch_store(store));
                }
            }
            Section::Accounts | Section::Name => {}
        }

        Step::Stay
    }

    /// Makes one change; true when it was made, else its failure is kept.
    fn change(&mut self, change: impl FnOnce(&mut Self) -> Result<(), Box<dyn Error>>) -> bool {
        self.failure = change(self).err().map(|error| {
            tracing::warn!("settings: {error}");
            error.to_string()
        });
        self.failure.is_none()
    }

    /// What the editor left of an Account's signature: `true` when
    /// `config.toml` changed, which the running Accounts read at start.
    pub fn signature_edited(&mut self, edited: Result<bool, Box<dyn Error>>) {
        self.change(|settings| {
            settings.reload |= edited?;
            let config = PartialConfig::load(&settings.dir.join("config.toml"))?;
            settings.signatures = signatures(&config);
            Ok(())
        });
    }

    fn rename(&mut self, name: Option<String>) -> Result<(), Box<dyn Error>> {
        self.write(|config| config.set_name(name.as_deref()))?;
        self.name = name;
        self.reload = true;
        Ok(())
    }

    /// Copies every Account's secret to `to`, then names it in
    /// `config.toml`, then deletes the old copies: no step loses a secret.
    fn switch_store(&mut self, to: SecretStore) -> Result<(), Box<dyn Error>> {
        let from = self.secrets;
        let dir = &self.dir;
        let keys: Vec<_> = self
            .accounts
            .iter()
            .flat_map(|(email, _)| [email.clone(), smtp_key(email)])
            .collect();
        let copied = keys.iter().try_for_each(|key| {
            from.read(dir, key)?
                .map_or(Ok(()), |secret| to.write(dir, key, &secret))
        });

        let written = copied.and_then(|()| self.write(|config| config.set_secrets(to)));
        let (kept, dropped) = if written.is_ok() {
            (to, from)
        } else {
            (from, to)
        };
        for key in &keys {
            if let Err(error) = dropped.delete(&self.dir, key) {
                tracing::warn!("{key}: {error}");
            }
        }

        written?;
        self.secrets = kept;
        self.reload = true;
        Ok(())
    }

    /// Writes `config.toml` as it is now, with `change` made.
    fn write(&self, change: impl FnOnce(&mut ConfigFile)) -> Result<(), Box<dyn Error>> {
        let mut config = ConfigFile::load(&self.dir.join("config.toml"))?;
        change(&mut config);
        config.save()?;
        Ok(())
    }
}

/// The first line of each Account's signature, in the order of the Accounts.
fn signatures(config: &PartialConfig) -> Vec<Option<String>> {
    let each = config.accounts().map(|(email, _)| {
        let text = config.signature_of(email)?.text().ok()??;
        text.lines().next().map(str::to_owned)
    });
    each.collect()
}
