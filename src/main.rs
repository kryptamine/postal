mod cli;
mod config;
mod credential;
mod date_label;
mod header;
mod imap;
mod inbox;
mod inbox_view;
mod keymap;
mod logging;
mod message;
mod message_list;
mod panic_hook;
mod run;
mod secret;
mod theme;
mod truncate;
mod xdg;

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use crate::{
    config::Config,
    credential::Credential,
    imap::{ImapSession, MailServer},
    inbox::Inbox,
    theme::Theme,
};

/// The first run fetches this many of the newest Messages.
const WINDOW: u32 = 500;

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
    let theme = Theme::from_env();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let mut terminal = ratatui::init();
        let result = run::run(&mut terminal, &theme, load_inbox(&config, &password)).await;
        ratatui::restore();
        Ok(result?)
    })
}

async fn load_inbox(config: &Config, password: &Credential) -> Result<Inbox, imap::Error> {
    let mut session = ImapSession::open(config, password).await?;
    let inbox = session.inbox(WINDOW).await?;
    session.logout().await?;
    Ok(inbox)
}
