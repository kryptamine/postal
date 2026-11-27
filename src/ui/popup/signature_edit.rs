use std::path::Path;

use crate::{
    compose::signature_edit, ui::error::Error, ui::popup::settings::Settings,
    ui::state::screen::Screen, ui::terminal::tty::Tty,
};

/// The editor on the signature of the Account `email`, with `config.toml`
/// in `dir`; Settings, still open under it, learn how it went.
pub fn open(tty: &mut Tty, screen: &mut Screen, dir: &Path, email: &str) -> Result<(), Error> {
    let mut edited = Ok(false);
    tty.outside(|| {
        edited = signature_edit::edit(dir, email);
        Ok(())
    })?;
    if let Some(settings) = screen.over.get_mut::<Settings>() {
        settings.signature_edited(edited);
    }

    Ok(())
}
