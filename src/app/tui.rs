use std::error::Error;

use crate::{
    account::unlink,
    account::xdg,
    app::on_screen::on_screen,
    app::onboard,
    app::session::session,
    app::signed_in::opened,
    ui::attachment_file,
    ui::exit::Exit,
    ui::onboarding::{self, Outcome, Start},
};

/// The TUI, left for the Onboarding on `L` or from Settings, and opened
/// again after it; after Settings, with the Accounts the config names now.
pub fn run() -> Result<(), Box<dyn Error>> {
    let dir = xdg::config_dir()?;
    attachment_file::clear();
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
                    failure = onboard::sign_in_again(slot, &dir, config, sign_in).await?;

                    continue;
                }
                Exit::AddAccount => {
                    let (terminal, background) = slot.take()?;
                    let theme = onboard::onboarding_theme(&dir, background)?;
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
