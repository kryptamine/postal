use std::error::Error;

use crate::{
    account::unlink,
    account::xdg,
    app::beside::Linked,
    app::on_screen::on_screen,
    app::onboard,
    app::session::{Opening, session},
    app::signed_in::opened,
    cache,
    compose::forwarded,
    mail::sync_lock::SyncLock,
    ui::attachment_file,
    ui::exit::Exit,
    ui::onboarding::{self, Outcome, Start},
};

/// The TUI, left for the Onboarding on `L` or from Settings, and opened
/// again after it; after Settings, with the Accounts the config names now.
/// A `linked` one opens the link's Draft once it starts.
pub fn run(linked: Option<Linked>) -> Result<(), Box<dyn Error>> {
    let dir = xdg::config_dir()?;
    let (_lock, mut link) = match linked {
        Some(Linked { lock, link }) => (lock, Some(link)),
        None => (sync_lock()?, None),
    };

    attachment_file::clear();
    if let Err(error) = forwarded::sweep() {
        tracing::warn!("removing the files of forwards gone failed: {error}");
    }

    on_screen(async |slot| {
        let (mut configs, mut sign_ins) = opened(slot, &dir).await?;
        let mut failure = None;
        loop {
            let opening = Opening {
                failure,
                link: link.take(),
            };

            let session = session(slot, &dir, &configs, &sign_ins, opening);
            let exit = Box::pin(session).await?;
            cache::settle(&xdg::cache_dir()?);
            failure = None;
            match exit {
                Exit::Quit => return Ok(()),
                Exit::SignIn(account) => {
                    let (config, sign_in) = (&configs[account], &mut sign_ins[account]);
                    failure = onboard::sign_in_again(slot, &dir, config, sign_in).await?;

                    continue;
                }
                Exit::AddAccount => {
                    let (tty, background) = slot.take()?;
                    let theme = onboard::onboarding_theme(&dir, background)?;
                    let outcome =
                        onboarding::run(tty.terminal, tty.events, &theme, &dir, Start::Choose)
                            .await?;
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

/// The lock a folder sync holds while it runs, waited for: it would send
/// the Outbox too (ADR 0033).
#[expect(
    clippy::print_stderr,
    reason = "the screen is not taken yet, so stderr reaches the user"
)]
fn sync_lock() -> Result<SyncLock, Box<dyn Error>> {
    let dir = xdg::cache_dir()?;
    if let Some(lock) = SyncLock::try_take(&dir)? {
        return Ok(lock);
    }

    eprintln!("postal: waiting for a folder sync to finish");
    Ok(SyncLock::take(&dir)?)
}
