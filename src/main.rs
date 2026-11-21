mod accounts;
mod all_inboxes;
mod alloc_count;
mod ansi;
mod answering;
mod atomic_file;
mod attribution;
mod awaited_reply;
mod backfill;
mod backoff;
mod body;
mod body_fetch;
mod body_key;
mod body_text;
mod border;
mod browser;
mod cache;
mod cell_grid;
mod centred;
mod cli;
mod clipboard;
mod command_output;
mod composing;
mod config;
mod config_file;
mod correspondence;
mod correspondent;
mod correspondent_list;
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
mod frame_glyphs;
mod frame_padding;
mod header;
mod help_popup;
mod hint_line;
mod html_preview;
mod html_text;
mod icons;
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
mod local_draft;
mod local_drafts;
mod local_drafts_pane;
mod logging;
mod login;
mod looks;
mod mail_colours;
mod mail_look;
mod marks;
mod matched;
mod message;
mod message_file;
mod message_flags;
mod message_header;
mod message_list;
mod mode;
mod modified_utf7;
mod move_changes;
mod new_message;
mod notes;
mod oauth;
mod older_pages;
mod onboarding;
mod open_outside;
mod outage;
mod outbox;
mod outbox_flush;
mod outgoing;
mod outlines;
mod over;
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
mod provider;
mod quoted_html;
mod quoted_text;
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
mod routes;
mod run;
mod screen;
mod search;
mod secret;
mod secret_store;
mod senders;
mod sent_pass;
mod sequence_numbers;
mod settings;
mod settings_popup;
mod showing;
mod sign_in;
mod smtp;
mod snippet;
mod snippets;
mod spare_work;
mod stack;
mod stack_str;
mod status;
mod steering;
mod styled_line;
mod sync;
mod sync_state;
mod terminal_backend;
mod terminal_background;
mod terminal_slot;
mod text_part;
mod theme;
mod thread;
mod thread_fetch;
mod thread_pane;
mod thread_show;
mod toast;
mod transfer_encoding;
mod truncate;
mod undo;
mod unique_id;
mod unlink;
mod update;
mod view;
mod wake;
mod wrap;
mod xdg;

use std::{error::Error, path::Path, process::ExitCode};

use futures::{TryFutureExt, channel::mpsc::UnboundedReceiver};
use steering::{Ask, Steering};

use crate::{
    accounts::Accounts,
    body_fetch::Request,
    cache::Cache,
    cli::{Command, Correspondent, Thread},
    config::{Config, IconChoice},
    icons::Icons,
    imap::ImapSession,
    oauth::Tokens,
    older_pages::OlderPages,
    onboarding::{Outcome, Start},
    outbox::Outbox,
    outbox_flush::Sender,
    outgoing::Outgoing,
    partial_config::PartialConfig,
    previews::Previews,
    provider::Provider,
    reply_fetch::ReplyFetch,
    rgb::Rgb,
    routes::Routes,
    run::{Background, Exit},
    sign_in::SignIn,
    status::Status,
    terminal_slot::TerminalSlot,
    theme::Theme,
    update::Update,
};

fn main() -> ExitCode {
    if let Some(code) = render_process::serve() {
        return code;
    }

    let log = xdg::state_dir().map(|dir| logging::path(&dir));
    let args = cli::parse(log.as_deref().ok());
    match args.command {
        // A command beside the TUI leaves its log alone.
        Some(Command::Correspondent {
            verb: Correspondent::List(list),
        }) => answered(correspondent_list::run(&list)),
        Some(Command::Thread {
            verb: Thread::Show(show),
        }) => answered(thread_show::run(&show)),
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

            logging::flush();
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
    if let Err(error) = logging::init(log, verbose.into()) {
        eprintln!("postal: {error}");
        return ExitCode::FAILURE;
    }

    panic_hook::install(log.to_owned());
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
    let dir = xdg::config_dir()?;
    let email = on_screen(async |slot| onboard(slot, &dir, Start::Choose).await)?;
    println!("Signed in as {email}.");
    Ok(())
}

/// The TUI, left for the Onboarding on `L` or from Settings, and opened
/// again after it; after Settings, with the Accounts the config names now.
fn try_main() -> Result<(), Box<dyn Error>> {
    let dir = xdg::config_dir()?;
    on_screen(async |slot| {
        let (mut configs, mut sign_ins) = opened(slot, &dir).await?;
        let mut failure = None;
        loop {
            let exit = Box::pin(session((slot, &dir), (&configs, &sign_ins), failure)).await?;
            failure = None;
            match exit {
                Exit::Quit => return Ok(()),
                Exit::SignIn(account) => {
                    if let (Some(config), Some(sign_in)) =
                        (configs.get(account), sign_ins.get_mut(account))
                    {
                        failure = sign_in_again(slot, &dir, config, sign_in).await?;
                    }

                    continue;
                }
                Exit::AddAccount => {
                    let (terminal, background) = slot.take()?;
                    let theme = onboarding_theme(&dir, background)?;
                    let outcome = onboarding::run(terminal, &theme, &dir, Start::Choose).await?;
                    if let Outcome::Quit(Some(_)) = outcome {
                        failure = Some("sign-in failed");
                    }
                }
                Exit::Unlink(email) => {
                    if let Err(error) = unlink::unlink(&dir, &email) {
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
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn opened(
    slot: &mut TerminalSlot,
    dir: &Path,
) -> Result<(Vec<Config>, Vec<Option<SignIn>>), Box<dyn Error>> {
    let (configs, sign_ins): (Vec<_>, Vec<_>) = signed_in(slot, dir).await?.into_iter().unzip();
    Ok((configs, sign_ins.into_iter().map(Some).collect()))
}

/// The Onboarding for `config` alone; what failed, if it did.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn sign_in_again(
    slot: &mut TerminalSlot,
    dir: &Path,
    config: &Config,
    sign_in: &mut Option<SignIn>,
) -> Result<Option<&'static str>, Box<dyn Error>> {
    let (terminal, background) = slot.take()?;
    let theme = onboarding_theme(dir, background)?;
    let failed = match onboarding::run(terminal, &theme, dir, Start::again(config)).await? {
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
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn onboard(
    slot: &mut TerminalSlot,
    dir: &Path,
    start: Start<'_>,
) -> Result<String, Box<dyn Error>> {
    let (terminal, background) = slot.take()?;
    let theme = onboarding_theme(dir, background)?;
    match onboarding::run(terminal, &theme, dir, start).await? {
        Outcome::SignedIn(email) => Ok(email),
        Outcome::Quit(error) => Err(error.unwrap_or_else(|| login::Error::Cancelled.into())),
    }
}

/// The Accounts the TUI opens with; with none yet, the Onboarding runs
/// first. A lost secret shows it for that Account alone, and leaving it
/// runs without the Account, unless none is left.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
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
            match onboarding::run(terminal, &theme, dir, Start::again(&config)).await? {
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
        return Err(left.unwrap_or_else(|| login::Error::Cancelled.into()));
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
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn session(
    (slot, dir): (&mut TerminalSlot, &Path),
    (configs, sign_ins): (&[Config], &[Option<SignIn>]),
    failure: Option<&'static str>,
) -> Result<Exit, Box<dyn Error>> {
    let emails: Vec<&str> = configs.iter().map(|config| config.email.as_str()).collect();
    let caches = Cache::open(&xdg::cache_dir()?, &emails)?;
    // Without one, Compose says so when it is used.
    let outbox = Outbox::new(xdg::data_dir().ok());
    let (sender, updates) = futures::channel::mpsc::unbounded();
    let (mut asks, mut requests, mut works) = (Vec::new(), Vec::new(), Vec::new());
    for (account, (config, sign_in)) in configs.iter().zip(sign_ins).enumerate() {
        let (ask, asked) = futures::channel::mpsc::unbounded();
        let (request, requested) = futures::channel::mpsc::unbounded();
        asks.push(ask);
        requests.push(request);
        // Its channels close with their receivers: what is asked of it is dropped.
        let Some(sign_in) = sign_in else {
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
            first: account == 0,
        };

        let cache = &caches[account];
        works.push(Box::pin(served(
            config,
            cache,
            (asked, requested),
            sender,
            send,
        )));
    }

    let requests = Routes::new(requests);
    let older = OlderPages::new(requests.clone(), configs.len());
    let replies = ReplyFetch::new(requests.clone(), configs.len());
    let (terminal, background) = slot.take()?;
    let icons = configs
        .first()
        .map_or(IconChoice::Auto, |config| config.icons);
    let mut theme = Theme::from_env(background, Icons::choose(icons));
    let to_previews = (configs.len(), sender);
    let mut previews = Previews::start(requests, to_previews, theme.mail_look());
    let background = Background {
        work: futures::future::try_join_all(works).map_ok(drop),
        updates,
        older,
        replies,
        asks: Routes::new(asks),
        accounts: Accounts(configs),
        outbox: &outbox,
        statuses: sign_ins.iter().map(Option::as_ref).map(starting).collect(),
        failure,
        config_dir: dir,
    };

    Ok(run::run(terminal, &mut theme, &caches, &mut previews, background).await?)
}

/// The Status an Account starts in.
const fn starting(sign_in: Option<&SignIn>) -> Status {
    match sign_in {
        Some(_) => Status::Live,
        None => Status::SignedOut,
    }
}

/// One Account's connections until the screen stops asking; signed out,
/// it ends and leaves the others running.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
async fn served(
    config: &Config,
    cache: &Cache,
    (asked, requested): (UnboundedReceiver<Ask>, UnboundedReceiver<Request>),
    sender: Sender<'_>,
    send: impl Fn(Update),
) -> Result<(), sync::Error> {
    let report = |status| send(Update::Status(status));
    let synced = sync(
        config,
        cache,
        (Steering::new(asked), sender),
        (&send, report),
    );
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
