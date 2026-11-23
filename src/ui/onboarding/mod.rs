//! The Onboarding on its own screen: the Provider, then its sign-in, in the
//! browser or by a password, at the servers typed for a generic Provider,
//! checked on the IMAP server before anything is written.

pub mod field;
mod page;
mod server_form;
mod step;

use std::{convert::Infallible, error::Error, io, path::Path, pin::Pin, pin::pin};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;

use self::{
    field::Edit,
    page::Page,
    server_form::{Keyed, ServerForm},
    step::Step,
};
use crate::{
    account::autoconfig::Autoconfig,
    account::config::Config,
    account::credential::Credential,
    account::partial_config::PartialConfig,
    account::provider::Provider,
    mail::discover::{self, Found},
    mail::login::{self, Consent, Granted},
    ui::frame::theme::Theme,
    ui::outside::clipboard,
    ui::terminal::input::Input,
    ui::terminal::terminal_backend::TerminalBackend,
};

pub enum Start<'a> {
    /// From the Provider list.
    Choose,
    /// Straight to the sign-in of this Account at its Provider.
    Again(&'a Config),
}

pub enum Outcome {
    /// Signed in as this Account, written.
    SignedIn(String),
    /// The user left, after this failure when there was one.
    Quit(Option<Box<dyn Error>>),
}

enum Attempt {
    SignedIn(String),
    Back,
    Cancelled,
    Failed(Box<dyn Error>),
}

#[derive(Clone, Copy)]
enum Key {
    Up,
    Down,
    Enter,
    Esc,
    /// `q`.
    Quit,
    /// `Ctrl-C`, a key in raw mode.
    Cancel,
    Char(char),
    Other,
}

impl Key {
    const fn of(event: KeyEvent) -> Self {
        let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
        match event.code {
            KeyCode::Char('c') if ctrl => Self::Cancel,
            _ if ctrl => Self::Other,
            KeyCode::Up | KeyCode::Char('k') => Self::Up,
            KeyCode::Down | KeyCode::Char('j') => Self::Down,
            KeyCode::Enter => Self::Enter,
            KeyCode::Esc => Self::Esc,
            KeyCode::Char('q') => Self::Quit,
            KeyCode::Char(c) => Self::Char(c),
            _ => Self::Other,
        }
    }
}

enum Waited<T> {
    Done(T),
    Key(KeyEvent),
}

/// A text field of a sign-in by password at `provider`.
#[derive(Clone, Copy)]
enum Field<'h> {
    Address {
        provider: Provider,
    },
    Password {
        heading: &'h str,
        provider: Provider,
    },
}

impl<'h> Field<'h> {
    fn step<'v>(self, value: &'v str) -> Step<'v>
    where
        'h: 'v,
    {
        match self {
            Self::Address { provider } => Step::Address { value, provider },
            Self::Password { heading, provider } => Step::Password {
                heading,
                length: value.chars().count(),
                provider,
            },
        }
    }

    fn accepts(self, value: &str) -> bool {
        match self {
            Self::Address { .. } => value.contains('@'),
            Self::Password { .. } => !value.is_empty(),
        }
    }
}

/// How a text field was left.
enum Typed {
    Entered,
    Back,
    Cancelled,
}

struct Screen<'t> {
    terminal: &'t mut Terminal<TerminalBackend>,
    theme: &'t Theme,
    events: &'t mut Input,
}

impl Screen<'_> {
    /// Shows `step` until `work` is done or a key is pressed.
    async fn wait<F: Future>(
        &mut self,
        step: &Step<'_>,
        mut work: Pin<&mut F>,
    ) -> io::Result<Waited<F::Output>> {
        loop {
            let theme = self.theme;
            self.terminal
                .draw(|frame| frame.render_widget(Page { theme, step }, frame.area()))?;
            tokio::select! {
                done = work.as_mut() => return Ok(Waited::Done(done)),
                event = self.events.next() => match event {
                    Some(Ok(Event::Key(event))) if event.kind == KeyEventKind::Press => {
                        return Ok(Waited::Key(event));
                    }
                    // A resize, say: drawn again.
                    Some(Ok(_)) => {}
                    Some(Err(error)) => return Err(error),
                    None => return Err(io::ErrorKind::UnexpectedEof.into()),
                },
            }
        }
    }

    async fn event(&mut self, step: &Step<'_>) -> io::Result<KeyEvent> {
        match self
            .wait(step, pin!(std::future::pending::<Infallible>()))
            .await?
        {
            Waited::Key(event) => Ok(event),
            Waited::Done(never) => match never {},
        }
    }

    async fn key(&mut self, step: &Step<'_>) -> io::Result<Key> {
        Ok(Key::of(self.event(step).await?))
    }

    /// `None` when the user quit.
    async fn choose(&mut self) -> io::Result<Option<Provider>> {
        let last = Provider::ALL.len() - 1;
        let mut selected = 0;
        loop {
            match self.key(&Step::Providers { selected }).await? {
                Key::Up => selected = selected.saturating_sub(1),
                Key::Down => selected = (selected + 1).min(last),
                Key::Enter => return Ok(Provider::ALL.get(selected).copied()),
                Key::Esc | Key::Quit | Key::Cancel => return Ok(None),
                Key::Char(_) | Key::Other => {}
            }
        }
    }

    /// The code the browser came back with, or how the attempt ended.
    async fn code(
        &mut self,
        consent: &Consent,
        provider: Provider,
        back: bool,
    ) -> io::Result<Result<Credential, Attempt>> {
        let heading = format!("Sign in to {} in the browser.", provider.service());
        let mut waiting = pin!(consent.code());
        let mut copied = false;
        loop {
            let step = Step::Browser {
                heading: &heading,
                url: &consent.url,
                copied,
                back,
            };

            match self.wait(&step, waiting.as_mut()).await? {
                Waited::Done(Ok(code)) => return Ok(Ok(code)),
                Waited::Done(Err(error)) => return Ok(Err(Attempt::Failed(error.into()))),
                Waited::Key(event) => match Key::of(event) {
                    Key::Char('c') => {
                        clipboard::copy(&consent.url)?;
                        copied = true;
                    }
                    Key::Esc => return Ok(Err(Attempt::Back)),
                    Key::Quit | Key::Cancel => return Ok(Err(Attempt::Cancelled)),
                    _ => {}
                },
            }
        }
    }

    /// Edits `value` in `field` until it is entered, as `field` accepts it, or left.
    async fn field(&mut self, value: &mut String, field: Field<'_>) -> io::Result<Typed> {
        loop {
            let event = self.event(&field.step(value)).await?;
            match field::edit(value, event) {
                Edit::Submit if field.accepts(value.trim()) => return Ok(Typed::Entered),
                Edit::Back => return Ok(Typed::Back),
                Edit::Cancel => return Ok(Typed::Cancelled),
                Edit::Submit | Edit::Changed | Edit::Ignored => {}
            }
        }
    }

    /// Edits `form` until it is entered with servers it accepts, or left.
    async fn servers(&mut self, form: &mut ServerForm, heading: &str) -> io::Result<Typed> {
        form.problem = None;
        loop {
            let event = self.event(&Step::Servers { heading, form }).await?;
            match form.key(event) {
                Keyed::Submit => return Ok(Typed::Entered),
                Keyed::Back => return Ok(Typed::Back),
                Keyed::Cancel => return Ok(Typed::Cancelled),
                Keyed::Stay => {}
            }
        }
    }

    /// The servers published for `email`, or none once `Esc` skips the
    /// lookup; `None` on `Ctrl-C`.
    async fn look_up(&mut self, email: &str, dir: &Path) -> io::Result<Option<Found>> {
        let sources = match PartialConfig::load(&dir.join("config.toml")) {
            Ok(partial) => partial.autoconfig,
            // The sign-in that follows reports it.
            Err(_unread) => Autoconfig::default(),
        };

        let heading = format!("Looking up the servers for {email}…");
        // Boxed: the lookups would make every Onboarding future too large.
        let mut looking = Box::pin(discover::discover(email, &sources));
        loop {
            match self
                .wait(&Step::LookingUp { heading: &heading }, looking.as_mut())
                .await?
            {
                Waited::Done(found) => return Ok(Some(found)),
                Waited::Key(event) => match Key::of(event) {
                    Key::Esc => {
                        tracing::info!("server lookup skipped");
                        return Ok(Some(Found::default()));
                    }
                    Key::Cancel => return Ok(None),
                    _ => {}
                },
            }
        }
    }

    /// Whether the user agrees to `question`: `y` does, `n`, `Enter` or `Esc` declines.
    async fn replaces(&mut self, question: &str) -> io::Result<bool> {
        loop {
            match self.key(&Step::Replace { question }).await? {
                Key::Char('y' | 'Y') => return Ok(true),
                Key::Char('n' | 'N') | Key::Enter | Key::Esc | Key::Quit | Key::Cancel => {
                    return Ok(false);
                }
                Key::Up | Key::Down | Key::Char(_) | Key::Other => {}
            }
        }
    }

    /// Whether the user tries again after `error`.
    async fn retries(&mut self, error: &str) -> io::Result<bool> {
        loop {
            match self.key(&Step::Failed { error }).await? {
                Key::Enter => return Ok(true),
                Key::Esc | Key::Quit | Key::Cancel => return Ok(false),
                Key::Up | Key::Down | Key::Char(_) | Key::Other => {}
            }
        }
    }
}

/// Runs on `terminal` until the user signs in or leaves; a failure is shown
/// and may be tried again.
pub async fn run(
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut Input,
    theme: &Theme,
    dir: &Path,
    start: Start<'_>,
) -> io::Result<Outcome> {
    let mut screen = Screen {
        terminal,
        theme,
        events,
    };

    let (choosing, hint, again) = match start {
        Start::Choose => (true, None, Provider::default()),
        Start::Again(config) => (false, Some(config.email.as_str()), config.provider),
    };

    // Kept across tries, so a refused password need not be typed with them again.
    let mut address = hint.unwrap_or_default().to_owned();
    let mut form = match start {
        Start::Again(config) => Some(ServerForm::of(&config.email, &config.imap, &config.smtp)),
        Start::Choose => None,
    };

    let mut failure: Option<Box<dyn Error>> = None;
    loop {
        if let Some(error) = &failure {
            if !screen.retries(&error.to_string()).await? {
                return Ok(Outcome::Quit(failure));
            }

            failure = None;
        }

        let provider = if choosing {
            match screen.choose().await? {
                Some(provider) => provider,
                None => return Ok(Outcome::Quit(None)),
            }
        } else {
            again
        };

        let attempt = match provider {
            Provider::Google => browser(&mut screen, dir, hint, choosing).await?,
            Provider::ICloud => password(&mut screen, dir, &mut address, choosing).await?,
            Provider::Other => other(&mut screen, dir, &mut address, &mut form, choosing).await?,
        };

        match attempt {
            Attempt::SignedIn(email) => return Ok(Outcome::SignedIn(email)),
            Attempt::Back if choosing => {}
            Attempt::Back | Attempt::Cancelled => return Ok(Outcome::Quit(None)),
            Attempt::Failed(error) => {
                tracing::warn!("sign-in failed: {error}");
                failure = Some(error);
            }
        }
    }
}

/// One sign-in at Google in the browser, offering the Account `hint`;
/// `back` when `Esc` returns to the Provider list.
async fn browser(
    screen: &mut Screen<'_>,
    dir: &Path,
    hint: Option<&str>,
    back: bool,
) -> io::Result<Attempt> {
    let consent = match Consent::open(dir, hint).await {
        Ok(consent) => consent,
        Err(error) => return Ok(Attempt::Failed(error)),
    };

    let code = match screen.code(&consent, Provider::Google, back).await? {
        Ok(code) => code,
        Err(left) => return Ok(left),
    };

    settle(screen, dir, pin!(consent.grant(&code))).await
}

/// One sign-in at iCloud by an app-specific password, as `address`, which
/// is asked for from the Provider list alone: otherwise it is the Account's.
async fn password(
    screen: &mut Screen<'_>,
    dir: &Path,
    address: &mut String,
    choosing: bool,
) -> io::Result<Attempt> {
    let mut password = String::new();
    loop {
        if choosing {
            let field = Field::Address {
                provider: Provider::ICloud,
            };

            match screen.field(address, field).await? {
                Typed::Entered => {}
                Typed::Back => return Ok(Attempt::Back),
                Typed::Cancelled => return Ok(Attempt::Cancelled),
            }
        }

        let email = address.trim().to_owned();
        let heading = format!("Sign in to iCloud as {email}");
        let field = Field::Password {
            heading: &heading,
            provider: Provider::ICloud,
        };

        match screen.field(&mut password, field).await? {
            Typed::Entered => {
                let secret = Credential::new(password.trim().to_owned());
                let checking = login::check_password(dir, Provider::ICloud, email, secret, None);
                return settle(screen, dir, pin!(checking)).await;
            }
            Typed::Back if choosing => password.clear(),
            Typed::Back => return Ok(Attempt::Back),
            Typed::Cancelled => return Ok(Attempt::Cancelled),
        }
    }
}

/// Where a sign-in at a generic Provider is.
enum Stage {
    Address,
    Servers,
    Password,
}

/// One sign-in at a generic Provider as `address`, at the servers of `form`,
/// looked up when there is none for the address yet.
async fn other(
    screen: &mut Screen<'_>,
    dir: &Path,
    address: &mut String,
    form: &mut Option<ServerForm>,
    choosing: bool,
) -> io::Result<Attempt> {
    let provider = Provider::Other;
    let mut password = String::new();
    let mut stage = if choosing {
        Stage::Address
    } else {
        Stage::Servers
    };

    loop {
        let email = address.trim().to_owned();
        let typed = match stage {
            Stage::Address => screen.field(address, Field::Address { provider }).await?,
            Stage::Servers => {
                let form = match form {
                    Some(form) if form.is_for(&email) => form,
                    _ => match screen.look_up(&email, dir).await? {
                        Some(found) => form.insert(ServerForm::found(&email, found)),
                        None => return Ok(Attempt::Cancelled),
                    },
                };

                let heading = format!("Servers for {email}");
                screen.servers(form, &heading).await?
            }
            Stage::Password => {
                let heading = format!("Sign in as {email}");
                let field = Field::Password {
                    heading: &heading,
                    provider,
                };

                screen.field(&mut password, field).await?
            }
        };

        stage = match (stage, typed) {
            (_, Typed::Cancelled) => return Ok(Attempt::Cancelled),
            (Stage::Address, Typed::Back) => return Ok(Attempt::Back),
            (Stage::Servers, Typed::Back) if !choosing => return Ok(Attempt::Back),
            (Stage::Servers, Typed::Back) => Stage::Address,
            (Stage::Address, Typed::Entered) | (Stage::Password, Typed::Back) => Stage::Servers,
            (Stage::Servers, Typed::Entered) => Stage::Password,
            (Stage::Password, Typed::Entered) => {
                let servers = form.as_ref().map(ServerForm::servers);
                let Some(Ok(servers)) = servers else {
                    stage = Stage::Servers;
                    continue;
                };

                let secret = Credential::new(password.trim().to_owned());
                let checking = login::check_password(dir, provider, email, secret, Some(servers));
                return settle(screen, dir, pin!(checking)).await;
            }
        };
    }
}

/// Waits for `granting` to sign in to the IMAP server, asks before it
/// replaces another Account, then writes it.
async fn settle<F>(
    screen: &mut Screen<'_>,
    dir: &Path,
    mut granting: Pin<&mut F>,
) -> io::Result<Attempt>
where
    F: Future<Output = Result<Granted, Box<dyn Error>>>,
{
    let granted = loop {
        match screen.wait(&Step::Checking, granting.as_mut()).await? {
            Waited::Done(Ok(granted)) => break granted,
            Waited::Done(Err(error)) => return Ok(Attempt::Failed(error)),
            Waited::Key(event) if matches!(Key::of(event), Key::Cancel) => {
                return Ok(Attempt::Cancelled);
            }
            Waited::Key(_) => {}
        }
    };

    if let Some(written) = granted.replaces() {
        let question = format!(
            "{} is signed in at {}. Sign in at {} instead and delete the Cache?",
            granted.email(),
            written.service(),
            granted.provider().service()
        );
        if !screen.replaces(&question).await? {
            return Ok(Attempt::Cancelled);
        }
    }

    let email = granted.email().to_owned();
    Ok(match granted.save(dir) {
        Ok(()) => Attempt::SignedIn(email),
        Err(error) => Attempt::Failed(error),
    })
}
