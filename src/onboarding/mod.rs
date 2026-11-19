//! The Onboarding on its own screen: the Provider, then its sign-in in the
//! browser, checked on the IMAP server before anything is written.

mod page;
mod step;

use std::{convert::Infallible, error::Error, io, path::Path, pin::Pin, pin::pin};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;

use self::{page::Page, step::Step};
use crate::{
    clipboard, credential::Credential, input::Input, login::Consent, provider::Provider,
    terminal_backend::TerminalBackend, theme::Theme,
};

pub enum Start<'a> {
    /// From the Provider list.
    Choose,
    /// Straight to the browser, which offers this Account first.
    Again(&'a str),
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
    Key(Key),
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
                        return Ok(Waited::Key(Key::of(event)));
                    }
                    // A resize, say: drawn again.
                    Some(Ok(_)) => {}
                    Some(Err(error)) => return Err(error),
                    None => return Err(io::ErrorKind::UnexpectedEof.into()),
                },
            }
        }
    }

    async fn key(&mut self, step: &Step<'_>) -> io::Result<Key> {
        match self
            .wait(step, pin!(std::future::pending::<Infallible>()))
            .await?
        {
            Waited::Key(key) => Ok(key),
            Waited::Done(never) => match never {},
        }
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
                Waited::Key(Key::Char('c')) => {
                    clipboard::copy(&consent.url)?;
                    copied = true;
                }
                Waited::Key(Key::Esc) => return Ok(Err(Attempt::Back)),
                Waited::Key(Key::Quit | Key::Cancel) => return Ok(Err(Attempt::Cancelled)),
                Waited::Key(_) => {}
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
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
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

    let (choosing, hint) = match start {
        Start::Choose => (true, None),
        Start::Again(email) => (false, Some(email)),
    };

    let mut failure: Option<Box<dyn Error>> = None;
    loop {
        if let Some(error) = &failure {
            if !screen.retries(&error.to_string()).await? {
                return Ok(Outcome::Quit(failure));
            }

            failure = None;
        }

        // The Provider signed in before, while only one exists to sign in to.
        let provider = if choosing {
            match screen.choose().await? {
                Some(provider) => provider,
                None => return Ok(Outcome::Quit(None)),
            }
        } else {
            Provider::Google
        };

        match attempt(&mut screen, dir, (provider, hint), choosing).await? {
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

/// One sign-in at `provider`, offering the Account `hint`; `back` when `Esc`
/// returns to the Provider list.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn attempt(
    screen: &mut Screen<'_>,
    dir: &Path,
    (provider, hint): (Provider, Option<&str>),
    back: bool,
) -> io::Result<Attempt> {
    let consent = match Consent::open(dir, hint).await {
        Ok(consent) => consent,
        Err(error) => return Ok(Attempt::Failed(error)),
    };

    let code = match screen.code(&consent, provider, back).await? {
        Ok(code) => code,
        Err(left) => return Ok(left),
    };

    let granted = {
        let mut granting = pin!(consent.grant(&code));
        loop {
            match screen.wait(&Step::Checking, granting.as_mut()).await? {
                Waited::Done(Ok(granted)) => break granted,
                Waited::Done(Err(error)) => return Ok(Attempt::Failed(error)),
                Waited::Key(Key::Cancel) => return Ok(Attempt::Cancelled),
                Waited::Key(_) => {}
            }
        }
    };

    if let Some(current) = granted.replaces() {
        let question = format!(
            "Signed in as {current}. Sign in as {} instead and delete the Cache?",
            granted.email()
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
