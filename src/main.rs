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
        }) => printed(&list, args.json),
        Some(Command::Thread {
            verb: Thread::Show(show),
        }) => printed(&show, args.json),
        Some(Command::Folder { verb }) => match verb {
            Folder::List(list) => printed(&list, args.json),
            Folder::Sync(sync) => printed(&sync, args.json),
        },
        Some(Command::Message { verb }) => match verb {
            Message::List(list) => printed(&list, args.json),
            Message::Show(show) => printed(&show, args.json),
            Message::Search(search) => printed(&search, args.json),
        },
        Some(Command::Mcp) => answered(cli::mcp::run()),
        Some(Command::Update) => answered(cli::update::run()),
        Some(Command::SetupMailto { remove }) => answered(cli::setup_mailto::run(remove)),
        Some(Command::OpenInTerminal { link }) => answered(app::in_terminal::open(&link)),
        tui => {
            let linked = match args.link.as_deref().map(compose::mailto::Mailto::parse) {
                None => None,
                Some(link) => match link.map_err(Into::into).and_then(app::beside::open) {
                    Ok(Some(linked)) => Some(linked),
                    done => return answered(done.map(drop)),
                },
            };

            let run: Box<dyn FnOnce() -> Result<(), Box<dyn Error>>> =
                if matches!(tui, Some(Command::Login)) {
                    Box::new(app::login::run)
                } else {
                    Box::new(|| app::tui::run(linked))
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

/// A command's answer on stdout, as `--json` asks; 1 when part of it failed.
fn printed(command: &impl cli::answer::Answers, json: bool) -> ExitCode {
    let printed = command.answer().and_then(|answer| {
        answer.print(json)?;
        Ok(answer.failed)
    });
    match printed {
        Ok(true) => ExitCode::FAILURE,
        printed => answered(printed.map(drop)),
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
fn logged(log: &Path, verbose: u8, run: impl FnOnce() -> Result<(), Box<dyn Error>>) -> ExitCode {
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
