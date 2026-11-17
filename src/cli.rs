use std::{ffi::OsString, fmt, path::Path};

/// The global flags; they are accepted anywhere on the command line.
pub struct Args {
    pub help: bool,
    /// How many times `-v` was given.
    pub verbosity: usize,
}

#[derive(Debug)]
pub struct UnknownArgument(OsString);

impl fmt::Display for UnknownArgument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown argument `{}`; see `postal --help`",
            self.0.display()
        )
    }
}

impl std::error::Error for UnknownArgument {}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Args, UnknownArgument> {
    let mut parsed = Args {
        help: false,
        verbosity: 0,
    };

    for arg in args {
        match arg.to_str() {
            Some("-h" | "--help") => parsed.help = true,
            Some(flag) if flag.len() > 1 && flag.strip_prefix('-').is_some_and(only_v) => {
                parsed.verbosity += flag.len() - 1;
            }
            _ => return Err(UnknownArgument(arg)),
        }
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
