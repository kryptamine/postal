mod account;
mod app;
mod cache;
mod cli;
mod compose;
mod domain;
mod mail;
mod render;
mod ui;

use std::{error::Error, path::Path, process::ExitCode};

use crate::cli::args::{Command, Correspondent, Thread};

fn main() -> ExitCode {
    if let Some(code) = render::render_process::serve() {
        return code;
    }

    let log = account::xdg::state_dir().map(|dir| ui::logging::path(&dir));
    let args = cli::args::parse(log.as_deref().ok());
    match args.command {
        // A command beside the TUI leaves its log alone.
        Some(Command::Correspondent {
            verb: Correspondent::List(list),
        }) => answered(cli::correspondent_list::run(&list)),
        Some(Command::Thread {
            verb: Thread::Show(show),
        }) => answered(cli::thread_show::run(&show)),
        tui => {
            let run = if matches!(tui, Some(Command::Login)) {
                app::login::run
            } else {
                app::tui::run
            };

            let code = match log {
                Ok(log) => logged(&log, args.verbose, run),
                Err(error) => answered(Err(error.into())),
            };

            ui::logging::flush();
            code
        }
    }
}

/// A command's end, its error on stderr.
#[expect(clippy::print_stderr, reason = "a command answers on stderr")]
fn answered(result: Result<(), Box<dyn Error>>) -> ExitCode {
    result.map_or_else(
        |error| {
            eprintln!("postal: {error}");
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}

#[expect(
    clippy::print_stderr,
    reason = "the terminal is restored, so stderr reaches the user"
)]
fn logged(log: &Path, verbose: u8, run: fn() -> Result<(), Box<dyn Error>>) -> ExitCode {
    if let Err(error) = ui::logging::init(log, verbose.into()) {
        eprintln!("postal: {error}");
        return ExitCode::FAILURE;
    }

    ui::panic_hook::install(log.to_owned());
    tracing::info!("postal {} starting", env!("CARGO_PKG_VERSION"));
    let result = run();
    match result {
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
