use std::{ffi::OsString, fmt, path::Path};

/// The global flags; they are accepted anywhere on the command line.
pub struct Args {
    pub help: bool,
    /// How many times `-v` was given.
    pub verbosity: usize,
    pub command: Command,
}

#[derive(Clone, Copy)]
pub enum Command {
    /// The TUI.
    Run,
    Login,
}

#[derive(Debug)]
pub enum Error {
    Unknown(OsString),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(arg) => write!(
                f,
                "unknown argument `{}`; see `postal --help`",
                arg.display()
            ),
        }
    }
}

impl std::error::Error for Error {}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Args, Error> {
    let mut parsed = Args {
        help: false,
        verbosity: 0,
        command: Command::Run,
    };

    let mut login = false;
    for arg in args {
        match arg.to_str() {
            Some("-h" | "--help") => parsed.help = true,
            Some("login") if !login => login = true,
            Some(flag) if flag.len() > 1 && flag.strip_prefix('-').is_some_and(only_v) => {
                parsed.verbosity += flag.len() - 1;
            }
            _ => return Err(Error::Unknown(arg)),
        }
    }

    if login {
        parsed.command = Command::Login;
    }

    Ok(parsed)
}

fn only_v(letters: &str) -> bool {
    letters.bytes().all(|b| b == b'v')
}

pub fn help(log: &Path) -> String {
    format!(
        "A keyboard-driven terminal mail client that reads one IMAP mailbox

Usage: postal [-v | -vv]
       postal login [-v | -vv]

Commands:
  login       choose a mail provider and sign in, adding an account

Options:
  -v          log debug events
  -vv         log trace events and the IMAP wire trace;
              trace contains headers, not bodies or credentials
  -h, --help  print this help

POSTAL_LOG overrides the level with an EnvFilter directive, such as `debug`.
Log: {}
",
        log.display()
    )
}
