use std::{backtrace::Backtrace, path::PathBuf, thread};

use crate::{ui::logging, ui::terminal::terminal_backend};

/// Logs the panic with its backtrace. One on the calling thread also
/// restores the terminal and points at the log, so `catch_unwind` there
/// would leave a broken screen; elsewhere the thread's owner deals with it.
#[expect(
    clippy::print_stderr,
    reason = "the terminal is restored, so stderr reaches the user"
)]
pub fn install(log: PathBuf) {
    let main = thread::current().id();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("{info}\n{}", Backtrace::force_capture());
        logging::flush();
        if thread::current().id() != main {
            return;
        }

        terminal_backend::restore();
        eprintln!("Postal crashed. Log: {}", log.display());
    }));
}
