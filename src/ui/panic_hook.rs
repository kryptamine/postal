use std::{backtrace::Backtrace, path::PathBuf};

use crate::ui::logging;

/// Restores the terminal, logs the panic with its backtrace, and points at the log.
#[expect(
    clippy::print_stderr,
    reason = "the terminal is restored, so stderr reaches the user"
)]
pub fn install(log: PathBuf) {
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        tracing::error!("{info}\n{}", Backtrace::force_capture());
        logging::flush();
        eprintln!("Postal crashed. Log: {}", log.display());
    }));
}
