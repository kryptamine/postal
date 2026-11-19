mod alloc_count;
mod ansi;
mod answering;
mod atomic_file;
mod awaited_reply;
mod backfill;
mod backoff;
mod body;
mod body_fetch;
mod body_key;
mod border;
mod browser;
mod cache;
mod cell_grid;
mod centred;
mod cli;
mod composing;
mod config;
mod credential;
mod date_group;
mod date_label;
mod draft;
mod draft_file;
mod draft_pane;
mod draft_preview;
mod dwell;
mod editor;
mod flag;
mod flag_change;
mod flag_changes;
mod folder;
mod folder_list;
mod folder_picker;
mod footer;
mod header;
mod help_popup;
mod hint_line;
mod html_preview;
mod html_text;
mod idle;
mod imap;
mod inbox;
mod inbox_view;
mod input;
mod keychain;
mod keymap;
mod layout;
mod link_hints;
mod listing;
mod litehtml;
mod logging;
mod login;
mod looks;
mod mail_colours;
mod marks;
mod matched;
mod message;
mod message_list;
mod mode;
mod modified_utf7;
mod move_changes;
mod new_message;
mod notes;
mod oauth;
mod older_pages;
mod outage;
mod outbox;
mod outbox_flush;
mod outgoing;
mod outlines;
mod pager;
mod pager_text;
mod paint;
mod palette;
mod panic_hook;
mod partial_config;
mod picker_popup;
mod preview;
mod preview_pane;
mod preview_render;
mod previews;
mod prompt;
mod reading;
mod reconnect;
mod region;
mod render_process;
mod rendered_previews;
mod rendered_view;
mod reply_draft;
mod reply_fetch;
mod rgb;
mod role_folder;
mod run;
mod screen;
mod search;
mod secret;
mod senders;
mod sequence_numbers;
mod shown;
mod sign_in;
mod smtp;
mod snippet;
mod snippets;
mod stack;
mod stack_str;
mod status;
mod steering;
mod styled_line;
mod suspend;
mod sync;
mod sync_state;
mod terminal_backend;
mod terminal_background;
mod text_part;
mod theme;
mod thread;
mod thread_fetch;
mod thread_pane;
mod toast;
mod transfer_encoding;
mod truncate;
mod undo;
mod unique_id;
mod update;
mod view;
mod wake;
mod wrap;
mod xdg;

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use futures::{TryFutureExt, channel::mpsc::UnboundedReceiver};
use steering::Steering;

use crate::{
    body_fetch::Request,
    cache::Cache,
    cli::Command,
    config::{Auth, Config, SecretStore},
    imap::ImapSession,
    oauth::Tokens,
    older_pages::OlderPages,
    outbox::Outbox,
    outbox_flush::Sender,
    outgoing::Outgoing,
    previews::Previews,
    reply_fetch::ReplyFetch,
    run::{Background, Exit, Work},
    sign_in::SignIn,
    status::Status,
    theme::Theme,
    update::Update,
};

#[expect(
    clippy::print_stderr,
    reason = "the terminal is restored, so stderr reaches the user"
)]
fn main() -> ExitCode {
    if let Some(code) = render_process::serve() {
        return code;
    }

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
#[expect(clippy::print_stdout, reason = "--help prints to stdout")]
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

#[expect(
    clippy::print_stderr,
    reason = "the terminal is restored, so stderr reaches the user"
)]
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
    // Without one, Compose says so when it is used.
    let outbox = Outbox::new(xdg::data_dir().ok());
    let outgoing = Outgoing { config };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let (sender, updates) = futures::channel::mpsc::unbounded();
    let (requests, requested) = futures::channel::mpsc::unbounded();
    let (asks, asked) = futures::channel::mpsc::unbounded();
    let older = OlderPages::new(requests.clone());
    let replies = ReplyFetch::new(requests.clone());
    let updates_to_previews = sender.clone();
    let send = move |update| {
        // The send fails only once the event loop has stopped listening.
        sender.unbounded_send(update).ok();
    };

    runtime.block_on(async {
        let mut terminal = terminal_backend::init()?;
        // A terminal that does not say is taken for a dark one.
        let background = terminal_background::query().unwrap_or(rgb::BLACK);
        let theme = Theme::from_env(background);
        let mut previews = Previews::start(requests, updates_to_previews, theme.mail);
        let work = sign_in.map_or(Work::SignedOut { failed }, |sign_in| {
            Work::Running(
                futures::future::try_join(
                    sync(
                        config,
                        &cache,
                        (
                            Steering::new(asked),
                            Sender {
                                outbox: &outbox,
                                outgoing,
                                sign_in,
                            },
                        ),
                        (&send, |status| send(Update::Status(status))),
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
            replies,
            asks,
            outgoing,
            outbox: &outbox,
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
        // Every run starts in the Inbox: its first Preview waits on no EXAMINE.
        sync::examine(&mut session, folder::INBOX).await?;
        // Asked for before an outage: the Preview asks again once told Live.
        while requests.try_recv().is_ok() {}
        report(Status::Live);
        let examined = Some(folder::INBOX.into());
        body_fetch::serve(&mut session, cache, (&mut requests, examined), &send).await
    };

    // The status slot follows the sync connection alone, but for a refused Account.
    reconnect::retry(attempt, |status| match status {
        Status::Live => send(Update::BodyLink(true)),
        Status::Retrying { .. } => send(Update::BodyLink(false)),
        Status::SignedOut => send(Update::Status(status)),
        Status::Syncing { .. } | Status::Connecting | Status::Fetching => {}
    })
    .await
}

#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
/// Resyncs, IDLEs and sends the Outbox on one connection, as the sequence
/// numbers IDLE sends count in its `EXAMINE`, on the Folder `steering`
/// follows. `send` follows every Cache write, `report` the connection.
async fn sync(
    config: &Config,
    cache: &Cache,
    (mut steering, sender): (Steering, Sender<'_>),
    (send, report): (&impl Fn(Update), impl Fn(Status)),
) -> Result<(), sync::Error> {
    let attempt = async |report: &dyn Fn(Status)| {
        let session = ImapSession::open(config, sender.sign_in).await?;
        idle::listen(session, cache, (&mut steering, sender), send, report).await
    };

    reconnect::retry(attempt, report).await
}
