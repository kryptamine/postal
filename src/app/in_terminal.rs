use std::{env, error::Error, path::Path, process::Command};

use crate::{
    account::config::{Terminal, TerminalApp},
    account::partial_config::PartialConfig,
    account::run::run,
    account::xdg,
    app::link_script,
};

/// Opens `postal <link>` in the terminal `config.toml` chooses. The link
/// stays one argument all the way, and no shell parses it (ADR 0034).
pub fn open(link: &str) -> Result<(), Box<dyn Error>> {
    let config = PartialConfig::load(&xdg::config_dir()?.join("config.toml"))?;
    let postal = env::current_exe()?;
    let command = match config.terminal {
        Terminal::Command(own) => {
            let mut command = Command::new(own);
            command.arg(&postal).arg(link);
            command
        }
        Terminal::App(TerminalApp::Ghostty) => given("Ghostty", &["-e"], &postal, link),
        Terminal::App(TerminalApp::Kitty) => given("kitty", &[], &postal, link),
        Terminal::App(TerminalApp::Wezterm) => given("WezTerm", &["start", "--"], &postal, link),
        // Neither takes a command as arguments, only a file to run.
        Terminal::App(TerminalApp::Terminal) => opened("Terminal", &postal, link)?,
        Terminal::App(TerminalApp::Iterm2) => opened("iTerm", &postal, link)?,
    };

    run(command)
}

/// A new instance of the macOS app `name`, given `before`, then `postal`
/// and `link`.
fn given(name: &str, before: &[&str], postal: &Path, link: &str) -> Command {
    let mut command = open_app(&["-na", name, "--args"]);
    command.args(before).arg(postal).arg(link);
    command
}

/// The macOS app `name` on a script that runs `postal` on `link`.
fn opened(name: &str, postal: &Path, link: &str) -> Result<Command, Box<dyn Error>> {
    let mut command = open_app(&["-a", name]);
    command.arg(link_script::write(link, postal)?);
    Ok(command)
}

fn open_app(flags: &[&str]) -> Command {
    let mut command = Command::new("open");
    command.args(flags);
    command
}
