use std::{env, error::Error, path::PathBuf};

// One handler per system; the other is compiled only for its lints.
#[cfg(target_os = "macos")]
use crate::account::handler_app as platform;
#[cfg(not(target_os = "macos"))]
use crate::account::handler_desktop as platform;

/// Whether the system lets Settings choose the terminal a link opens in;
/// a Linux desktop entry asks the desktop for its own.
pub const CHOOSES_TERMINAL: bool = platform::CHOOSES_TERMINAL;

/// Whether this system hands mailto: links to Postal, as far as the files
/// Postal wrote for it say.
pub fn is_set_up() -> bool {
    platform::path().is_ok_and(|path| path.exists())
}

/// Makes the running postal the system's mailto: handler. Returns the
/// app or desktop entry it installed.
pub fn set_up() -> Result<PathBuf, Box<dyn Error>> {
    platform::install(&env::current_exe()?)
}

/// Removes what `set_up` installed, so the system goes back to the
/// handler before it. Returns what it removed, if anything was there.
pub fn remove() -> Result<Option<PathBuf>, Box<dyn Error>> {
    platform::remove()
}
