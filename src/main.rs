mod config;
mod date_label;
mod header;
mod imap;
mod inbox;
mod inbox_view;
mod keymap;
mod message;
mod message_list;
mod run;
mod secret;
mod theme;
mod truncate;
mod xdg;

use std::process::ExitCode;

use crate::{
    config::Config,
    imap::{ImapSession, MailServer},
    inbox::Inbox,
    theme::Theme,
};

/// The first run fetches this many of the newest Messages.
const WINDOW: u32 = 500;

fn main() -> ExitCode {
    match try_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("postal: {error}");
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

async fn load_inbox(config: &Config, password: &str) -> Result<Inbox, imap::Error> {
    let mut session = ImapSession::open(config, password).await?;
    let inbox = session.inbox(WINDOW).await?;
    session.logout().await?;
    Ok(inbox)
}
