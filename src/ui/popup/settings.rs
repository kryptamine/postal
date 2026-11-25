use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Duration,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    account::config::{
        self, ComposeFormat, ContactsChoice, IconChoice, NotifyChoice, SecretStore, Terminal,
    },
    account::config_file::ConfigFile,
    account::mailto_handler::{self, CHOOSES_TERMINAL},
    account::partial_config::PartialConfig,
    account::provider::Provider,
    account::sign_in::smtp_key,
    ui::onboarding::field::{self, Edit},
    ui::popup::settings_mode::{Field, Mode},
    ui::popup::settings_section::Section,
    ui::popup::settings_value::Value,
};

/// One row of a Settings tab the selection can rest on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// The Account at this place in `config.toml`.
    Account(usize),
    /// The signature of the Account at this place.
    Signature(usize),
    Add,
    Choice(Value),
    Name,
    SendDelay,
    /// Whether mailto: links open in Postal.
    Handler,
    /// The program a mailto: link runs in place of a terminal.
    TerminalCommand,
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
    Contacts(ContactsChoice),
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
    pub send_delay: Duration,
    pub secrets: SecretStore,
    pub terminal: Terminal,
    pub contacts: ContactsChoice,
    pub compose_format: ComposeFormat,
    /// Whether the system hands mailto: links to Postal.
    pub handler: bool,
    section: Section,
    /// Index into the tab's rows.
    selected: usize,
    mode: Mode,
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
            send_delay: config.send_delay,
            secrets: config.secrets,
            terminal: config.terminal,
            contacts: config.contacts,
            compose_format: config.compose_format,
            handler: mailto_handler::is_set_up(),
            section: Section::Accounts,
            selected: 0,
            mode: Mode::Browsing,
            failure: None,
            reload: false,
        })
    }

    pub const fn mode(&self) -> &Mode {
        &self.mode
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
        let first = (section == Section::Mailto).then_some(Row::Handler);
        let accounts = (section == Section::Accounts)
            .then_some(0..self.accounts.len())
            .into_iter()
            .flatten()
            .flat_map(|account| [Row::Account(account), Row::Signature(account)]);
        let values = Value::of(section).iter().copied().map(Row::Choice);
        let last = match section {
            Section::Accounts => Some(Row::Add),
            Section::Name => Some(Row::Name),
            Section::Send => Some(Row::SendDelay),
            Section::Mailto if CHOOSES_TERMINAL => Some(Row::TerminalCommand),
            Section::Icons
            | Section::Notify
            | Section::Secrets
            | Section::Mailto
            | Section::Contacts => None,
        };
        first.into_iter().chain(accounts).chain(values).chain(last)
    }

    pub fn selected(&self) -> Row {
        self.rows().nth(self.selected).unwrap_or(Row::Add)
    }

    pub fn in_effect(&self, value: Value) -> bool {
        match value {
            Value::Icons(icons) => icons == self.icons,
            Value::Notify(notify) => notify == self.notify,
            Value::Secrets(store) => store == self.secrets,
            Value::Terminal(app) => self.terminal == Terminal::App(app),
            Value::Contacts(contacts) => contacts == self.contacts,
            Value::Compose(format) => format == self.compose_format,
        }
    }

    /// Shows the next tab or the one before, its value in effect selected.
    fn switch(&mut self, forward: bool) {
        self.section = self.section.next(forward);
        let in_effect = |row| matches!(row, Row::Choice(value) if self.in_effect(value));
        let at = self.rows().position(in_effect);
        self.selected = at.unwrap_or(0);
        self.failure = None;
    }

    pub fn key(&mut self, key: KeyEvent) -> Step {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            return Step::Quit;
        }

        match std::mem::replace(&mut self.mode, Mode::Browsing) {
            Mode::Browsing => {}
            Mode::Typing(typing, mut typed) => {
                match field::edit(&mut typed, key) {
                    Edit::Submit => {
                        let text = Some(typed.trim().to_owned()).filter(|text| !text.is_empty());
                        match typing {
                            Field::Name => self.change(|settings| settings.rename(text)),
                            Field::SendDelay => {
                                self.change(|settings| settings.set_send_delay(text.as_deref()))
                            }
                            Field::TerminalCommand => {
                                let terminal =
                                    text.map_or_else(Terminal::default, Terminal::Command);
                                self.change(|settings| settings.set_terminal(terminal))
                            }
                        };
                    }
                    Edit::Back => {}
                    Edit::Cancel => return Step::Quit,
                    Edit::Changed | Edit::Ignored => self.mode = Mode::Typing(typing, typed),
                }

                return Step::Stay;
            }
            Mode::Unlinking(email) => {
                return match key.code {
                    KeyCode::Char('y' | 'Y') => Step::Unlink(email),
                    _ => Step::Stay,
                };
            }
        }

        if ctrl {
            return Step::Stay;
        }

        let last = self.rows().count().saturating_sub(1);
        match key.code {
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => self.switch(true),
            KeyCode::BackTab | KeyCode::Char('h') | KeyCode::Left => self.switch(false),
            KeyCode::Char('j') | KeyCode::Down => self.selected = (self.selected + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('d') | KeyCode::Backspace => {
                if let Row::Account(account) = self.selected() {
                    self.failure = None;
                    self.mode = Mode::Unlinking(self.accounts[account].0.clone());
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
            Row::Name => {
                self.mode = Mode::Typing(Field::Name, self.name.clone().unwrap_or_default());
            }
            Row::SendDelay => {
                let seconds = self.send_delay.as_secs().to_string();
                self.mode = Mode::Typing(Field::SendDelay, seconds);
            }
            Row::TerminalCommand => {
                let command = match &self.terminal {
                    Terminal::Command(command) => command.clone(),
                    Terminal::App(_) => String::new(),
                };

                self.mode = Mode::Typing(Field::TerminalCommand, command);
            }
            Row::Handler => {
                self.change(Self::switch_handler);
            }
            Row::Choice(value) => return self.choose(value),
        }

        Step::Stay
    }

    /// Puts `value` in effect, unless it is already.
    fn choose(&mut self, value: Value) -> Step {
        if self.in_effect(value) {
            return Step::Stay;
        }

        match value {
            Value::Icons(icons) => {
                if self.change(|settings| settings.write(|config| config.set_icons(icons))) {
                    self.icons = icons;
                    return Step::Icons(icons);
                }
            }
            Value::Notify(notify) => {
                if self.change(|settings| settings.write(|config| config.set_notify(notify))) {
                    self.notify = notify;
                    return Step::Notify(notify);
                }
            }
            Value::Secrets(store) => {
                self.change(|settings| settings.switch_store(store));
            }
            Value::Terminal(app) => {
                self.change(|settings| settings.set_terminal(Terminal::App(app)));
            }
            Value::Compose(format) => {
                self.change(|settings| settings.set_compose_format(format));
            }
            Value::Contacts(contacts) => {
                if self.change(|settings| settings.write(|config| config.set_contacts(contacts))) {
                    self.contacts = contacts;
                    return Step::Contacts(contacts);
                }
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

    /// `typed` whole seconds, or the default for none.
    fn set_send_delay(&mut self, typed: Option<&str>) -> Result<(), Box<dyn Error>> {
        let delay = match typed {
            Some(typed) => Duration::from_secs(
                typed
                    .parse()
                    .map_err(|error| format!("the delay is whole seconds: {error}"))?,
            ),
            None => config::SEND_DELAY,
        };

        self.write(|config| config.set_send_delay(delay))?;
        self.send_delay = delay;
        // The Outbox holds each Send for the delay it opened with.
        self.reload = true;
        Ok(())
    }

    fn set_compose_format(&mut self, format: ComposeFormat) -> Result<(), Box<dyn Error>> {
        self.write(|config| config.set_compose_format(format))?;
        self.compose_format = format;
        // Each Account read its format at start.
        self.reload = true;
        Ok(())
    }

    fn set_terminal(&mut self, terminal: Terminal) -> Result<(), Box<dyn Error>> {
        self.write(|config| config.set_terminal(&terminal))?;
        self.terminal = terminal;
        Ok(())
    }

    /// Sets Postal up as the mailto: handler, or removes it.
    fn switch_handler(&mut self) -> Result<(), Box<dyn Error>> {
        if self.handler {
            mailto_handler::remove()?;
        } else {
            mailto_handler::set_up()?;
        }

        self.handler = !self.handler;
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
