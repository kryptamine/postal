use std::{
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
};

/// The global flags; they are accepted anywhere on the command line.
pub struct Args {
    pub help: bool,
    /// How many times `-v` was given.
    pub verbosity: usize,
    pub command: Command,
}

pub enum Command {
    /// The TUI.
    Run,
    Login,
    CorrespondentList(CorrespondentList),
}

/// `postal correspondent list`, its Account given by a Draft or by address.
#[derive(Default)]
pub struct CorrespondentList {
    pub draft: Option<PathBuf>,
    pub account: Option<String>,
    pub json: bool,
}

#[derive(Debug)]
pub enum Error {
    Unknown(OsString),
    /// A flag given without the value it takes.
    Missing(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(arg) => write!(
                f,
                "unknown argument `{}`; see `postal --help`",
                arg.display()
            ),
            Self::Missing(flag) => write!(f, "`{flag}` takes a value; see `postal --help`"),
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

    let (mut words, mut list) = (Vec::new(), CorrespondentList::default());
    // The first flag of `correspondent list`, refused with any other command.
    let mut listing = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => parsed.help = true,
            Some("--json") => {
                list.json = true;
                listing.get_or_insert_with(|| arg.clone());
            }
            Some("--draft") => {
                list.draft = Some(args.next().ok_or(Error::Missing("--draft"))?.into());
                listing.get_or_insert(arg);
            }
            Some("--account") => {
                let account = args.next().ok_or(Error::Missing("--account"))?;
                list.account = Some(account.to_string_lossy().into_owned());
                listing.get_or_insert(arg);
            }
            Some(flag) if flag.len() > 1 && flag.strip_prefix('-').is_some_and(only_v) => {
                parsed.verbosity += flag.len() - 1;
            }
            Some(word) if !word.starts_with('-') => words.push(arg),
            _ => return Err(Error::Unknown(arg)),
        }
    }

    let named: Vec<_> = words
        .iter()
        .map(|w| w.to_str().unwrap_or_default())
        .collect();
    parsed.command = match named.as_slice() {
        [] => Command::Run,
        ["login"] => Command::Login,
        ["correspondent", "list"] => {
            return Ok(Args {
                command: Command::CorrespondentList(list),
                ..parsed
            });
        }
        [..] => return Err(Error::Unknown(words.pop().unwrap_or_default())),
    };

    listing.map_or(Ok(parsed), |flag| Err(Error::Unknown(flag)))
}

fn only_v(letters: &str) -> bool {
    letters.bytes().all(|b| b == b'v')
}

pub fn help(log: &Path) -> String {
    format!(
        "A keyboard-driven terminal mail client that reads one IMAP mailbox

Usage: postal [-v | -vv]
       postal login [-v | -vv]
       postal correspondent list [--draft <path> | --account <email>] [--json]

Commands:
  login                choose a mail provider and sign in, adding an account
  correspondent list   every address the accounts have mail from or wrote to,
                       best first, as the Cache knows them; the account of
                       --draft's Draft, or --account, first

Options:
  --json      print a command's answer as a JSON array
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
