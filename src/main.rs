mod alloc_count;
mod ansi;
mod atomic_file;
mod backfill;
mod backoff;
mod body_fetch;
mod body_key;
mod browser;
mod cache;
mod centred;
mod cli;
mod config;
mod credential;
mod date_label;
mod header;
mod help_popup;
mod hint_line;
mod html_preview;
mod html_text;
mod idle;
mod imap;
mod inbox;
mod inbox_view;
mod keychain;
mod keymap;
mod layout;
mod link_hints;
mod logging;
mod login;
mod marks;
mod message;
mod message_list;
mod mode;
mod new_message;
mod oauth;
mod older_pages;
mod outage;
mod pager;
mod pager_text;
mod panic_hook;
mod partial_config;
mod preview;
mod preview_pane;
mod preview_render;
mod previews;
mod prompt;
mod reconnect;
mod rendered_previews;
mod run;
mod secret;
mod sequence_numbers;
mod shown;
mod sign_in;
mod snippet;
mod snippets;
mod stack_str;
mod status;
mod styled_line;
mod suspend;
mod sync;
mod sync_state;
mod terminal_backend;
mod text_part;
mod theme;
mod toast;
mod transfer_encoding;
mod truncate;
mod update;
mod wake;
mod wrap;
mod xdg;

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use futures::{TryFutureExt, channel::mpsc::UnboundedReceiver};

use crate::{
    body_fetch::Request,
    cache::Cache,
    cli::Command,
    config::{Auth, Config, SecretStore},
    imap::{ImapSession, MailServer},
    oauth::Tokens,
    older_pages::OlderPages,
    previews::Previews,
    run::{Background, Exit, Work},
    sign_in::SignIn,
    status::Status,
    theme::Theme,
    update::Update,
};

fn main() -> ExitCode {
    let (log, command) = match start() {
        Ok(Some(started)) => started,
        Ok(None) => return ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("postal: {error}");
            return ExitCode::FAILURE;
        }
    };

    let code = logged(&log, command);
    logging::flush();
    code
}

/// Parses the flags and opens the log; `None` once `--help` is printed.
fn start() -> Result<Option<(PathBuf, Command)>, Box<dyn std::error::Error>> {
    let args = cli::parse(std::env::args_os().skip(1))?;
    let log = logging::path(&xdg::state_dir()?);
    if args.help {
        print!("{}", cli::help(&log));
        return Ok(None);
    }

    logging::init(&log, args.verbosity)?;
    panic_hook::install(log.clone());
    Ok(Some((log, args.command)))
}

fn logged(log: &Path, command: Command) -> ExitCode {
    tracing::info!("postal {} starting", env!("CARGO_PKG_VERSION"));
    let result = match command {
        Command::Run => try_main(),
        Command::Login => xdg::config_dir()
            .map_err(Into::into)
            .and_then(|dir| login::oauth(&dir, None)),
        Command::LoginAppPassword => xdg::config_dir()
            .map_err(Into::into)
            .and_then(|dir| login::app_password(&dir, None)),
    };

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

/// The TUI, left for the login flow on `L` and opened again after it.
fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = xdg::config_dir()?;
    let (mut config, sign_in) = signed_in(&dir)?;
    let mut sign_in = Some(sign_in);
    let mut failed = false;
    loop {
        match session(&config, sign_in.as_ref(), failed)? {
            Exit::Quit => return Ok(()),
            Exit::SignIn => match login::sign_in_again(&dir, &config) {
                Ok(()) => {
                    let (signed_in_as, fresh) = signed_in(&dir)?;
                    (config, sign_in, failed) = (signed_in_as, Some(fresh), false);
                }
                Err(error) => {
                    sign_in = None;
                    failed = !login::cancelled(&*error);
                    if failed {
                        tracing::error!("signing in again failed: {error}");
                    }
                }
            },
        }
    }
}

/// The Account the TUI opens as; with none yet, the login flow runs first.
fn signed_in(dir: &Path) -> Result<(Config, SignIn), Box<dyn std::error::Error>> {
    let config_path = dir.join("config.toml");
    loop {
        let Some(config) = Config::load(&config_path)? else {
            login::oauth(dir, None)?;
            continue;
        };

        let secret = match config.secrets {
            SecretStore::Keychain => keychain::read(&config.email, &config_path)?,
            SecretStore::File => secret::read(&dir.join("secret"))?,
        };

        let Some(secret) = secret else {
            login::sign_in_again(dir, &config)?;
            continue;
        };

        let sign_in = match config.auth {
            Auth::AppPassword => SignIn::Password(secret),
            Auth::OAuth => match Tokens::refreshed(secret, config.oauth.token_url.clone()) {
                Ok(tokens) => SignIn::OAuth(tokens),
                Err(error) if error.signed_out() => {
                    tracing::warn!("{error}");
                    login::sign_in_again(dir, &config)?;
                    continue;
                }
                Err(error) => return Err(error.into()),
            },
        };

        return Ok((config, sign_in));
    }
}

/// The TUI until it exits, signed out without `sign_in`; `failed` when the
/// last sign-in did.
fn session(
    config: &Config,
    sign_in: Option<&SignIn>,
    failed: bool,
) -> Result<Exit, Box<dyn std::error::Error>> {
    let cache = Cache::open(&xdg::cache_dir()?)?;
    let theme = Theme::from_env();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let (sender, updates) = futures::channel::mpsc::unbounded();
    let (requests, requested) = futures::channel::mpsc::unbounded();
    let older = OlderPages::new(requests.clone());
    let mut previews = Previews::start(requests, sender.clone());
    let send = move |update| {
        // The send fails only once the event loop has stopped listening.
        let _ = sender.unbounded_send(update);
    };

    runtime.block_on(async {
        let mut terminal = terminal_backend::init()?;
        let work = sign_in.map_or(Work::SignedOut { failed }, |sign_in| {
            Work::Running(
                futures::future::try_join(
                    sync(
                        config,
                        sign_in,
                        &cache,
                        || send(Update::Inbox),
                        |status| send(Update::Status(status)),
                    ),
                    bodies(config, sign_in, &cache, requested, &send),
                )
                .map_ok(|((), ())| ()),
            )
        });
        let background = Background {
            work,
            updates,
            older,
        };

        let result = run::run(&mut terminal, &theme, &cache, &mut previews, background).await;
        ratatui::restore();
        Ok(result?)
    })
}

/// The second connection, kept open for the bodies the Preview asks for:
/// opening one per fetch would add TLS and login to the first Preview.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn bodies(
    config: &Config,
    sign_in: &SignIn,
    cache: &Cache,
    mut requests: UnboundedReceiver<Request>,
    send: impl Fn(Update),
) -> Result<(), sync::Error> {
    let attempt = async |report: &dyn Fn(Status)| {
        let mut session = ImapSession::open(config, sign_in).await?;
        session.examine().await?;
        // Asked for before an outage: the Preview asks again once told Live.
        while requests.try_recv().is_ok() {}
        report(Status::Live);
        body_fetch::serve(&mut session, cache, &mut requests, &send).await
    };

    // The status slot follows the sync connection alone, but for a refused Account.
    reconnect::retry(attempt, |status| match status {
        Status::Live => send(Update::BodyLink(true)),
        Status::Retrying { .. } => send(Update::BodyLink(false)),
        Status::SignedOut => send(Update::Status(status)),
        Status::Syncing { .. } | Status::Connecting => {}
    })
    .await
}

#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
/// Resyncs and IDLEs on one connection, as the sequence numbers IDLE sends
/// count in its `EXAMINE`. `written` follows every Cache write, `report` the
/// connection.
async fn sync(
    config: &Config,
    sign_in: &SignIn,
    cache: &Cache,
    written: impl Fn(),
    report: impl Fn(Status),
) -> Result<(), sync::Error> {
    let attempt = async |report: &dyn Fn(Status)| {
        let session = ImapSession::open(config, sign_in).await?;
        idle::listen(session, cache, &written, report).await
    };

    reconnect::retry(attempt, report).await
}
