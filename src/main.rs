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

use crate::cli::args::{Command, Correspondent, Folder, Message, Thread};

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
        }) => printed(cli::correspondent_list::answer(&list), list.json),
        Some(Command::Thread {
            verb: Thread::Show(show),
        }) => printed(cli::thread_show::answer(&show), show.json),
        Some(Command::Folder {
            verb: Folder::List(list),
        }) => printed(cli::folder_list::answer(), list.json),
        Some(Command::Message { verb }) => match verb {
            Message::List(list) => printed(cli::message_list::answer(&list), list.json),
            Message::Show(show) => printed(cli::message_show::answer(&show), show.json),
            Message::Search(search) => printed(cli::message_search::answer(&search), search.json),
        },
        Some(Command::Mcp) => answered(cli::mcp::run()),
        Some(Command::Update) => answered(cli::update::run()),
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

/// A command's answer on stdout, as `--json` asks.
fn printed(answer: Result<cli::answer::Answer, Box<dyn Error>>, json: bool) -> ExitCode {
    answered(answer.and_then(|answer| Ok(answer.print(json)?)))
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
