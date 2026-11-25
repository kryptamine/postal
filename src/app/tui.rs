use std::{error::Error, path::Path};

use crate::{
    account::unlink,
    account::xdg,
    app::beside::Linked,
    app::moved,
    app::on_screen::on_screen,
    app::onboard,
    app::session::{Opening, session},
    app::signed_in::opened,
    cache,
    compose::forwarded,
    mail::held,
    mail::sync_lock::{Kind, SyncLock},
    ui::attachment_file,
    ui::exit::Exit,
    ui::move_here::move_here,
    ui::onboarding::{self, Outcome, Start},
    ui::terminal_slot::TerminalSlot,
};

/// The TUI, left for the Onboarding on `L` or from Settings, and opened
/// again after it; after Settings, with the Accounts the config names now.
/// A `linked` one opens the link's Draft once it starts.
#[expect(
    clippy::print_stderr,
    reason = "the screen is handed back, so stderr reaches the user"
)]
pub fn run(linked: Option<Linked>) -> Result<(), Box<dyn Error>> {
    let dir = xdg::config_dir()?;
    let (lock, mut link) = match linked {
        Some(Linked { lock, link }) => (Some(lock), Some(link)),
        None => (SyncLock::try_take(Kind::Tui)?, None),
    };

    let moved = on_screen(async |slot| {
        let Some(lock) = taken(slot, &dir, lock).await? else {
            return Ok(false);
        };

        hold(lock);
        let (mut configs, mut sign_ins) = opened(slot, &dir).await?;
        let mut failure = None;
        loop {
            let opening = Opening {
                failure,
                link: link.take(),
            };

            let session = session(slot, &dir, &configs, &sign_ins, opening);
            let exit = Box::pin(session).await?;
            // Let go with the program still open, the Cache is another's now.
            if !held::released() {
                cache::settle(&xdg::cache_dir()?);
            }

            failure = None;
            match exit {
                Exit::Quit => return Ok(false),
                Exit::Moved => return Ok(true),
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
    })?;

    if moved {
        eprintln!("{}", moved::line());
    }

    Ok(())
}

/// The lock: `lock`, else whatever holds it gives it up, as the user says.
async fn taken(
    slot: &mut TerminalSlot,
    dir: &Path,
    lock: Option<SyncLock>,
) -> Result<Option<SyncLock>, Box<dyn Error>> {
    if lock.is_some() {
        return Ok(lock);
    }

    let (tty, background) = slot.take()?;
    let theme = onboard::onboarding_theme(dir, background)?;
    Ok(move_here(tty, &theme).await?)
}

/// Keeps `lock` for a TUI in another terminal to ask for, then clears what
/// the last session left.
fn hold(lock: SyncLock) {
    held::hold(lock);
    moved::listen();
    attachment_file::clear();
    if let Err(error) = forwarded::sweep() {
        tracing::warn!("removing the files of forwards gone failed: {error}");
    }
}
