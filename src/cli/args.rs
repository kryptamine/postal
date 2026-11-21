use std::path::{Path, PathBuf};

use clap::{ArgAction, CommandFactory, FromArgMatches, Parser, Subcommand};

/// A keyboard-driven terminal mail client that reads one or more IMAP
/// mailboxes. Without a command, the TUI.
#[derive(Parser)]
#[command(name = "postal")]
pub struct Args {
    /// Log debug events; -vv, trace events and the IMAP wire trace: trace
    /// contains headers, not bodies or credentials
    #[arg(short, action = ArgAction::Count, global = true)]
    pub verbose: u8,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Choose a mail provider and sign in, adding an account
    Login,
    /// The addresses the accounts have mail from or wrote to
    Correspondent {
        #[command(subcommand)]
        verb: Correspondent,
    },
    /// The Messages of one conversation
    Thread {
        #[command(subcommand)]
        verb: Thread,
    },
}

#[derive(Subcommand)]
pub enum Correspondent {
    /// Every one the Cache knows, best first
    List(CorrespondentList),
}

#[derive(Subcommand)]
pub enum Thread {
    /// Every Message of one Thread the Cache holds, oldest first
    Show(ThreadShow),
}

#[derive(clap::Args)]
pub struct ThreadShow {
    /// The Thread of the Message this Reply's Draft answers
    #[arg(long, value_name = "PATH")]
    pub draft: PathBuf,
    /// Print a JSON array, whose fields only grow
    #[arg(long)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct CorrespondentList {
    /// Rank first the account this Draft goes out from
    #[arg(long, value_name = "PATH", conflicts_with = "account")]
    pub draft: Option<PathBuf>,
    /// Rank first this account
    #[arg(long, value_name = "EMAIL")]
    pub account: Option<String>,
    /// Print a JSON array, whose fields only grow
    #[arg(long)]
    pub json: bool,
}

/// The arguments, or exits after printing help or what is wrong with them;
/// help names `log`, the TUI's log, when it is known.
pub fn parse(log: Option<&Path>) -> Args {
    let after = log.map_or_else(String::new, |log| {
        format!(
            "POSTAL_LOG overrides the level with an EnvFilter directive, such as `debug`.\n\
             Log: {}",
            log.display()
        )
    });

    let matches = Args::command().after_help(after).get_matches();
    Args::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
}
