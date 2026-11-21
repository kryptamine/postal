mod account;
mod cache;
mod cli;
mod compose;
mod domain;
mod mail;
mod render;
mod ui;

use std::{error::Error, path::Path, process::ExitCode};

use futures::{TryFutureExt, channel::mpsc::UnboundedReceiver};

use crate::{
    account::config::{Config, IconChoice},
    account::oauth::Tokens,
    account::partial_config::PartialConfig,
    account::provider::Provider,
    account::sign_in::SignIn,
    cache::Cache,
    cli::args::{Command, Correspondent, Thread},
    compose::accounts::Accounts,
    compose::outbox::Outbox,
    compose::outgoing::Outgoing,
    domain::per_account::PerAccount,
    mail::body_fetch::Request,
    mail::imap::ImapSession,
    mail::outbox_flush::Sender,
    mail::reply_fetch::ReplyFetch,
    mail::routes::Routes,
    mail::status::Status,
    mail::steering::{Ask, Steering},
    mail::tell::Tell,
    mail::update::Update,
    render::rgb::Rgb,
    ui::icons::Icons,
    ui::older_pages::OlderPages,
    ui::onboarding::{Outcome, Start},
    ui::previews::Previews,
    ui::run::{Background, Exit},
    ui::terminal_slot::TerminalSlot,
    ui::theme::Theme,
};

fn main() -> ExitCode {
    if let Some(code) = render::render_process::serve() {
        return code;
    }

    let log = account::xdg::state_dir().map(|dir| ui::logging::path(&dir));
    let args = cli::args::parse(log.as_deref().ok());
    match args.command {
        // A command beside the TUI leaves its log alone.
        Some(Command::Correspondent {
            verb: Correspondent::List(list),
        }) => answered(cli::correspondent_list::run(&list)),
        Some(Command::Thread {
            verb: Thread::Show(show),
        }) => answered(cli::thread_show::run(&show)),
        tui => {
            let run = if matches!(tui, Some(Command::Login)) {
                login
            } else {
                try_main
            };
            let code = match log {
                Ok(log) => logged(&log, args.verbose, run),
                Err(error) => answered(Err(error.into())),
            };

            ui::logging::flush();
            code
        }
    }
}

/// A command's end, its error on stderr.
#[expect(clippy::print_stderr, reason = "a command answers on stderr")]
fn answered(result: Result<(), Box<dyn Error>>) -> ExitCode {
    result.map_or_else(
        |error| {
            eprintln!("postal: {error}");
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}

#[expect(
    clippy::print_stderr,
    reason = "the terminal is restored, so stderr reaches the user"
)]
fn logged(log: &Path, verbose: u8, run: fn() -> Result<(), Box<dyn Error>>) -> ExitCode {
    if let Err(error) = ui::logging::init(log, verbose.into()) {
        eprintln!("postal: {error}");
        return ExitCode::FAILURE;
    }

    ui::panic_hook::install(log.to_owned());
    tracing::info!("postal {} starting", env!("CARGO_PKG_VERSION"));
    let result = run();
    match result {
        Ok(()) => {
            tracing::info!("exiting");
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!("{error}");
            eprintln!("postal: {error}\nLog: {}", log.display());
            ExitCode::FAILURE
        }
    }
}

/// `postal login`: the Onboarding from the Provider list.
#[expect(
    clippy::print_stdout,
    reason = "the terminal is restored, so stdout reaches the user"
)]
fn login() -> Result<(), Box<dyn Error>> {
    let dir = account::xdg::config_dir()?;
    let email = on_screen(async |slot| onboard(slot, &dir, Start::Choose).await)?;
    println!("Signed in as {email}.");
    Ok(())
}

/// The TUI, left for the Onboarding on `L` or from Settings, and opened
/// again after it; after Settings, with the Accounts the config names now.
fn try_main() -> Result<(), Box<dyn Error>> {
    let dir = account::xdg::config_dir()?;
    on_screen(async |slot| {
        let (mut configs, mut sign_ins) = opened(slot, &dir).await?;
        let mut failure = None;
        loop {
            let exit = Box::pin(session(slot, &dir, &configs, &sign_ins, failure)).await?;
            failure = None;
            match exit {
                Exit::Quit => return Ok(()),
                Exit::SignIn(account) => {
                    let (config, sign_in) = (&configs[account], &mut sign_ins[account]);
                    failure = sign_in_again(slot, &dir, config, sign_in).await?;

                    continue;
                }
                Exit::AddAccount => {
                    let (terminal, background) = slot.take()?;
                    let theme = onboarding_theme(&dir, background)?;
                    let outcome =
                        ui::onboarding::run(terminal, &theme, &dir, Start::Choose).await?;
                    if let Outcome::Quit(Some(_)) = outcome {
                        failure = Some("sign-in failed");
                    }
                }
                Exit::Unlink(email) => {
                    if let Err(error) = account::unlink::unlink(&dir, &email) {
                        tracing::error!("unlinking {email}: {error}");
                        failure = Some("unlink failed");
                    }
                }
                Exit::Reload => {}
            }

            (configs, sign_ins) = opened(slot, &dir).await?;
        }
    })
}

/// The Accounts the config names, each with its sign-in.
async fn opened(
    slot: &mut TerminalSlot,
    dir: &Path,
) -> Result<(PerAccount<Config>, PerAccount<Option<SignIn>>), Box<dyn Error>> {
    let (configs, sign_ins): (Vec<_>, Vec<_>) = signed_in(slot, dir).await?.into_iter().unzip();
    Ok((
        configs.into_iter().collect(),
        sign_ins.into_iter().map(Some).collect(),
    ))
}

/// The Onboarding for `config` alone; what failed, if it did.
async fn sign_in_again(
    slot: &mut TerminalSlot,
    dir: &Path,
    config: &Config,
    sign_in: &mut Option<SignIn>,
) -> Result<Option<&'static str>, Box<dyn Error>> {
    let (terminal, background) = slot.take()?;
    let theme = onboarding_theme(dir, background)?;
    let failed = match ui::onboarding::run(terminal, &theme, dir, Start::again(config)).await? {
        Outcome::SignedIn(_) => {
            *sign_in = read_sign_in(config, dir)?;
            sign_in.is_none()
        }
        Outcome::Quit(error) => {
            *sign_in = None;
            error.is_some()
        }
    };

    Ok(failed.then_some("sign-in failed"))
}

/// Runs `work`, which takes the terminal once it draws, and hands the
/// terminal back as Postal found it.
fn on_screen<T>(
    work: impl AsyncFnOnce(&mut TerminalSlot) -> Result<T, Box<dyn Error>>,
) -> Result<T, Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async { work(&mut TerminalSlot::new()).await })
}

/// The Theme before there is an Account: the config may set the icons.
fn onboarding_theme(dir: &Path, background: Rgb) -> Result<Theme, Box<dyn Error>> {
    let icons = PartialConfig::load(&dir.join("config.toml"))?.icons;
    Ok(Theme::from_env(background, Icons::choose(icons)))
}

/// The Onboarding from `start`; leaving it is an error, the last failure
/// or a cancel.
async fn onboard(
    slot: &mut TerminalSlot,
    dir: &Path,
    start: Start<'_>,
) -> Result<String, Box<dyn Error>> {
    let (terminal, background) = slot.take()?;
    let theme = onboarding_theme(dir, background)?;
    match ui::onboarding::run(terminal, &theme, dir, start).await? {
        Outcome::SignedIn(email) => Ok(email),
        Outcome::Quit(error) => Err(error.unwrap_or_else(|| mail::login::Error::Cancelled.into())),
    }
}

/// The Accounts the TUI opens with; with none yet, the Onboarding runs
/// first. A lost secret shows it for that Account alone, and leaving it
/// runs without the Account, unless none is left.
async fn signed_in(
    slot: &mut TerminalSlot,
    dir: &Path,
) -> Result<Vec<(Config, SignIn)>, Box<dyn Error>> {
    let config_path = dir.join("config.toml");
    let mut configs = Config::load(&config_path)?;
    if configs.is_empty() {
        onboard(slot, dir, Start::Choose).await?;
        configs = Config::load(&config_path)?;
    }

    let mut accounts = Vec::new();
    let mut left = None;
    for config in configs {
        loop {
            if let Some(sign_in) = read_sign_in(&config, dir)? {
                accounts.push((config, sign_in));
                break;
            }

            let (terminal, background) = slot.take()?;
            let theme = onboarding_theme(dir, background)?;
            match ui::onboarding::run(terminal, &theme, dir, Start::again(&config)).await? {
                Outcome::SignedIn(_) => {}
                Outcome::Quit(error) => {
                    tracing::info!("running without {}", config.email);
                    left = error;
                    break;
                }
            }
        }
    }

    if accounts.is_empty() {
        return Err(left.unwrap_or_else(|| mail::login::Error::Cancelled.into()));
    }

    Ok(accounts)
}

/// How `config` signs in, from its secret; `None` once that is lost.
fn read_sign_in(config: &Config, dir: &Path) -> Result<Option<SignIn>, Box<dyn Error>> {
    let Some(secret) = config.secrets.read(dir, &config.email)? else {
        return Ok(None);
    };

    Ok(Some(match config.provider {
        Provider::ICloud => SignIn::Password(secret),
        Provider::Google => match Tokens::refreshed(secret, config.oauth.token_url.clone()) {
            Ok(tokens) => SignIn::OAuth(tokens),
            Err(error) if error.signed_out() => {
                tracing::warn!("{error}");
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        },
    }))
}

/// The TUI until it exits, each Account of `configs` signed out without
/// its sign-in; `failure` toasts what failed before it.
async fn session(
    slot: &mut TerminalSlot,
    dir: &Path,
    configs: &PerAccount<Config>,
    sign_ins: &PerAccount<Option<SignIn>>,
    failure: Option<&'static str>,
) -> Result<Exit, Box<dyn Error>> {
    let emails = configs.map(|config| config.email.as_str());
    let caches = Cache::open(&account::xdg::cache_dir()?, &emails)?;
    // Without one, Compose says so when it is used.
    let outbox = Outbox::new(account::xdg::data_dir().ok());
    let (sender, updates) = futures::channel::mpsc::unbounded();
    let (mut asks, mut requests, mut works) = (Vec::new(), Vec::new(), Vec::new());
    for (account, config) in configs.iter() {
        let (ask, asked) = futures::channel::mpsc::unbounded();
        let (request, requested) = futures::channel::mpsc::unbounded();
        asks.push(ask);
        requests.push(request);
        // Its channels close with their receivers: what is asked of it is dropped.
        let Some(sign_in) = &sign_ins[account] else {
            continue;
        };

        let sender = sender.clone();
        let send = move |update| {
            // The send fails only once the event loop has stopped listening.
            sender.unbounded_send((account, update)).ok();
        };

        let sender = Sender {
            outbox: &outbox,
            outgoing: Outgoing { config },
            sign_in,
            first: configs.first() == Some(account),
        };

        let cache = &caches[account];
        works.push(Box::pin(served(cache, asked, requested, sender, send)));
    }

    let requests = Routes::new(requests.into_iter().collect());
    let older = OlderPages::new(requests.clone(), configs.map(|_| false));
    let replies = ReplyFetch::new(requests.clone(), configs.map(|_| false));
    let (terminal, background) = slot.take()?;
    let icons = configs
        .values()
        .next()
        .map_or(IconChoice::Auto, |config| config.icons);
    let mut theme = Theme::from_env(background, Icons::choose(icons));
    let mut previews = Previews::start(requests, configs, sender, theme.mail_look());
    let background = Background {
        work: futures::future::try_join_all(works).map_ok(drop),
        updates,
        older,
        replies,
        asks: Routes::new(asks.into_iter().collect()),
        accounts: Accounts(configs),
        outbox: &outbox,
        statuses: sign_ins.map(|sign_in| starting(sign_in.as_ref())),
        failure,
        config_dir: dir,
    };

    Ok(ui::run::run(terminal, &mut theme, &caches, &mut previews, background).await?)
}

/// The Status an Account starts in.
const fn starting(sign_in: Option<&SignIn>) -> Status {
    match sign_in {
        Some(_) => Status::Live,
        None => Status::SignedOut,
    }
}

/// The connections of the Account `sender` sends from until the screen
/// stops asking; signed out, they end and leave the others running.
async fn served(
    cache: &Cache,
    asked: UnboundedReceiver<Ask>,
    requested: UnboundedReceiver<Request>,
    sender: Sender<'_>,
    send: impl Fn(Update),
) -> Result<(), mail::sync::Error> {
    let config = sender.outgoing.config;
    let report = |status| send(Update::Status(status));
    let synced = sync(cache, Steering::new(asked), sender, &send, report);
    let fetched = bodies(config, sender.sign_in, cache, requested, &send);
    match futures::future::try_join(synced, fetched).await {
        Err(error) if error.signed_out() => {
            tracing::warn!("{}: {error}", config.email);
            Ok(())
        }
        result => result.map(drop),
    }
}

/// The second connection, kept open for the bodies the Preview asks for:
/// opening one per fetch would add TLS and login to the first Preview.
async fn bodies(
    config: &Config,
    sign_in: &SignIn,
    cache: &Cache,
    mut requests: UnboundedReceiver<Request>,
    send: impl Fn(Update),
) -> Result<(), mail::sync::Error> {
    let attempt = async |report: &dyn Fn(Status)| {
        let mut session = ImapSession::open(config, sign_in).await?;
        // Every run starts in the Inbox: its first Preview waits on no EXAMINE.
        mail::sync::examine(&mut session, domain::folder::INBOX).await?;
        // Asked for before an outage: the Preview asks again once told Live.
        while requests.try_recv().is_ok() {}
        report(Status::Live);
        let examined = Some(domain::folder::INBOX.into());
        mail::body_fetch::serve(&mut session, cache, &mut requests, examined, &send).await
    };

    // The status slot follows the sync connection alone, but for a refused Account.
    mail::reconnect::retry(attempt, |status| match status {
        Status::Live => send(Update::BodyLink(true)),
        Status::Retrying { .. } => send(Update::BodyLink(false)),
        Status::SignedOut => send(Update::Status(status)),
        Status::Syncing { .. } | Status::Connecting | Status::Fetching => {}
    })
    .await
}

/// Resyncs, IDLEs and sends the Outbox on one connection, as the sequence
/// numbers IDLE sends count in its `EXAMINE`, on the Folder `steering`
/// follows, for the Account `sender` sends from. `send` follows every Cache
/// write, `report` the connection.
async fn sync(
    cache: &Cache,
    mut steering: Steering,
    sender: Sender<'_>,
    send: &impl Fn(Update),
    report: impl Fn(Status),
) -> Result<(), mail::sync::Error> {
    let attempt = async |report: &dyn Fn(Status)| {
        let session = ImapSession::open(sender.outgoing.config, sender.sign_in).await?;
        let tell = Tell { send, report };
        mail::idle::listen(session, cache, &mut steering, sender, tell).await
    };

    mail::reconnect::retry(attempt, report).await
}
