mod alloc_count;
mod ansi;
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
mod keymap;
mod layout;
mod link_hints;
mod logging;
mod marks;
mod message;
mod message_list;
mod new_message;
mod outage;
mod pager;
mod pager_text;
mod panic_hook;
mod preview;
mod preview_pane;
mod preview_render;
mod previews;
mod reconnect;
mod rendered_previews;
mod run;
mod secret;
mod sequence_numbers;
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
    body_fetch::BodyRequest,
    cache::Cache,
    config::Config,
    credential::Credential,
    imap::{ImapSession, MailServer},
    previews::Previews,
    run::Background,
    status::Status,
    theme::Theme,
    update::Update,
};

fn main() -> ExitCode {
    let log = match start() {
        Ok(Some(log)) => log,
        Ok(None) => return ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("postal: {error}");
            return ExitCode::FAILURE;
        }
    };

    let code = logged(&log);
    logging::flush();
    code
}

/// Parses the flags and opens the log; `None` once `--help` is printed.
fn start() -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let args = cli::parse(std::env::args_os().skip(1))?;
    let log = logging::path(&xdg::state_dir()?);
    if args.help {
        print!("{}", cli::help(&log));
        return Ok(None);
    }

    logging::init(&log, args.verbosity)?;
    panic_hook::install(log.clone());
    Ok(Some(log))
}

fn logged(log: &Path) -> ExitCode {
    tracing::info!("postal {} starting", env!("CARGO_PKG_VERSION"));
    match try_main() {
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

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = xdg::config_dir()?;
    let config = Config::load(&dir.join("config.toml"))?;
    let password = secret::read(&dir.join("secret"))?;
    let cache = Cache::open(&xdg::cache_dir()?)?;
    let theme = Theme::from_env();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let (sender, updates) = futures::channel::mpsc::unbounded();
    let (requests, requested) = futures::channel::mpsc::unbounded();
    let mut previews = Previews::start(requests, sender.clone());
    let send = move |update| {
        // The send fails only once the event loop has stopped listening.
        let _ = sender.unbounded_send(update);
    };

    runtime.block_on(async {
        let mut terminal = terminal_backend::init()?;
        let work = futures::future::try_join(
            sync(
                &config,
                &password,
                &cache,
                || send(Update::Inbox),
                |status| send(Update::Status(status)),
            ),
            bodies(&config, &password, &cache, requested, |uids| {
                send(Update::Bodies(uids));
            }),
        );
        let background = Background {
            work: work.map_ok(|((), ())| ()),
            updates,
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
    password: &Credential,
    cache: &Cache,
    mut requests: UnboundedReceiver<BodyRequest>,
    stored: impl Fn(Vec<u32>),
) -> Result<(), sync::Error> {
    let mut waiting = Vec::new();
    let attempt = async |report: &dyn Fn(Status)| {
        let mut session = ImapSession::open(config, password).await?;
        session.examine().await?;
        report(Status::Live);
        body_fetch::serve(&mut session, cache, &mut requests, &mut waiting, &stored).await
    };

    // The status slot follows the sync connection alone.
    reconnect::retry(attempt, |_| {}).await
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
    password: &Credential,
    cache: &Cache,
    written: impl Fn(),
    report: impl Fn(Status),
) -> Result<(), sync::Error> {
    let attempt = async |report: &dyn Fn(Status)| {
        let session = ImapSession::open(config, password).await?;
        idle::listen(session, cache, &written, report).await
    };

    reconnect::retry(attempt, report).await
}
