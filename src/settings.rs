use std::{
    error::Error,
    path::{Path, PathBuf},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    config::{self, IconChoice, SecretStore},
    config_file::ConfigFile,
    onboarding::field::{self, Edit},
    partial_config::PartialConfig,
    provider::Provider,
};

/// One row of Settings the selection can rest on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// The Account at this place in `config.toml`.
    Account(usize),
    Add,
    Icons,
    Name,
    Secrets,
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
    Icons(IconChoice),
    Quit,
}

/// The settings of `config.toml`, each change written at once.
pub struct Settings {
    dir: PathBuf,
    pub accounts: Vec<(String, Provider)>,
    pub icons: IconChoice,
    pub name: Option<String>,
    pub secrets: SecretStore,
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
            accounts: config
                .accounts()
                .map(|(email, provider)| (email.to_owned(), provider))
                .collect(),
            icons: config.icons,
            name: config.name,
            secrets: config.secrets,
            selected: 0,
            editing: None,
            unlinking: None,
            failure: None,
            reload: false,
        })
    }

    pub fn rows(&self) -> impl Iterator<Item = Row> {
        (0..self.accounts.len()).map(Row::Account).chain([
            Row::Add,
            Row::Icons,
            Row::Name,
            Row::Secrets,
        ])
    }

    pub fn selected(&self) -> Row {
        self.rows().nth(self.selected).unwrap_or(Row::Add)
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

        let last = self.rows().count() - 1;
        match key.code {
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
            Row::Add => return Step::AddAccount,
            Row::Icons => {
                let icons = self.icons.next();
                if self.change(|settings| settings.write(|config| config.set_icons(icons))) {
                    self.icons = icons;
                    return Step::Icons(icons);
                }
            }
            Row::Name => self.editing = Some(self.name.clone().unwrap_or_default()),
            Row::Secrets => {
                self.change(Self::switch_store);
            }
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

    fn rename(&mut self, name: Option<String>) -> Result<(), Box<dyn Error>> {
        self.write(|config| config.set_name(name.as_deref()))?;
        self.name = name;
        self.reload = true;
        Ok(())
    }

    /// Copies every Account's secret to the other store, then names it in
    /// `config.toml`, then deletes the old copies: no step loses a secret.
    fn switch_store(&mut self) -> Result<(), Box<dyn Error>> {
        let (from, to) = (self.secrets, self.secrets.other());
        let dir = &self.dir;
        let copied = self.accounts.iter().try_for_each(|(email, _)| {
            from.read(dir, email)?
                .map_or(Ok(()), |secret| to.write(dir, email, &secret))
        });

        let written = copied.and_then(|()| self.write(|config| config.set_secrets(to)));
        let (kept, dropped) = if written.is_ok() {
            (to, from)
        } else {
            (from, to)
        };
        for (email, _) in &self.accounts {
            if let Err(error) = dropped.delete(&self.dir, email) {
                tracing::warn!("{email}: {error}");
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
