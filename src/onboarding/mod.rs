//! The Onboarding on its own screen: the Provider, then its sign-in, in the
//! browser or by an app-specific password, checked on the IMAP server before
//! anything is written.

pub mod field;
mod page;
mod step;

use std::{convert::Infallible, error::Error, io, path::Path, pin::Pin, pin::pin};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;

use self::{field::Edit, page::Page, step::Step};
use crate::{
    clipboard,
    config::Config,
    credential::Credential,
    input::Input,
    login::{self, Consent, Granted},
    provider::Provider,
    terminal_backend::TerminalBackend,
    theme::Theme,
};

pub enum Start<'a> {
    /// From the Provider list.
    Choose,
    /// Straight to the sign-in of this Account at its Provider.
    Again(&'a str, Provider),
}

impl<'a> Start<'a> {
    pub fn again(config: &'a Config) -> Self {
        Self::Again(&config.email, config.provider)
    }
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

/// A text field of the iCloud sign-in.
#[derive(Clone, Copy)]
enum Field<'h> {
    Address,
    Password { heading: &'h str },
}

impl<'h> Field<'h> {
    fn step<'v>(self, value: &'v str) -> Step<'v>
    where
        'h: 'v,
    {
        match self {
            Self::Address => Step::Address { value },
            Self::Password { heading } => Step::Password {
                heading,
                length: value.chars().count(),
            },
        }
    }

    fn accepts(self, value: &str) -> bool {
        match self {
            Self::Address => value.contains('@'),
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
    events: Input,
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
    theme: &Theme,
    dir: &Path,
    start: Start<'_>,
) -> io::Result<Outcome> {
    let mut screen = Screen {
        terminal,
        theme,
        events: Input::new(),
    };

    let (choosing, hint, again) = match start {
        Start::Choose => (true, None, Provider::default()),
        Start::Again(email, provider) => (false, Some(email), provider),
    };

    // Kept across tries, so a refused password need not be typed with it again.
    let mut address = hint.unwrap_or_default().to_owned();
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
            match screen.field(address, Field::Address).await? {
                Typed::Entered => {}
                Typed::Back => return Ok(Attempt::Back),
                Typed::Cancelled => return Ok(Attempt::Cancelled),
            }
        }

        let email = address.trim().to_owned();
        let heading = format!("Sign in to iCloud as {email}");
        let field = Field::Password { heading: &heading };
        match screen.field(&mut password, field).await? {
            Typed::Entered => {
                let secret = Credential::new(password.trim().to_owned());
                let checking = login::check_password(dir, Provider::ICloud, (email, secret));
                return settle(screen, dir, pin!(checking)).await;
            }
            Typed::Back if choosing => password.clear(),
            Typed::Back => return Ok(Attempt::Back),
            Typed::Cancelled => return Ok(Attempt::Cancelled),
        }
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
