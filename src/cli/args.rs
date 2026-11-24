use std::path::{Path, PathBuf};

use clap::{ArgAction, CommandFactory, FromArgMatches, Parser, Subcommand};
use schemars::JsonSchema;
use serde::Deserialize;

/// The Threads a listing holds when the arguments name no limit.
pub const LIMIT: usize = 50;

/// A keyboard-driven terminal mail client that reads one or more IMAP
/// mailboxes. Without a command, the TUI.
#[derive(Parser)]
#[command(name = "postal", version)]
pub struct Args {
    /// Log debug events; -vv, trace events and the IMAP wire trace: trace
    /// contains headers, not bodies or credentials
    #[arg(short, action = ArgAction::Count, global = true)]
    pub verbose: u8,
    /// Print a command's answer as a JSON array, whose fields only grow
    #[arg(long, global = true)]
    pub json: bool,
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
    /// The mailboxes of every account
    Folder {
        #[command(subcommand)]
        verb: Folder,
    },
    /// The mail the Cache holds
    Message {
        #[command(subcommand)]
        verb: Message,
    },
    /// Serve these commands to an agent as MCP tools on stdin and stdout
    Mcp,
    /// Replace this postal with the latest release
    Update,
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

#[derive(Subcommand)]
pub enum Folder {
    /// Every account's Folders, with their Message and Unread counts
    List(FolderList),
}

#[derive(Subcommand)]
pub enum Message {
    /// The newest Threads of a Folder, by default every account's Inbox
    List(MessageList),
    /// One Message with its body as plain text
    Show(MessageShow),
    /// The newest Threads with a Message whose words start with the query's
    Search(MessageSearch),
}

// Each struct of arguments is also its MCP tool's parameters (ADR 0030).
#[derive(clap::Args, Deserialize, JsonSchema)]
pub struct ThreadShow {
    /// The Thread's ID, as a listing names it
    #[arg(conflicts_with = "draft", required_unless_present = "draft")]
    pub id: Option<String>,
    /// The Thread of the Message this Reply's Draft answers
    #[arg(long, value_name = "PATH")]
    pub draft: Option<PathBuf>,
}

#[derive(clap::Args, Deserialize, JsonSchema)]
pub struct FolderList {}

#[derive(clap::Args, Deserialize, JsonSchema)]
pub struct MessageList {
    /// Only this Folder, by its name or its role, such as Sent
    #[arg(long, value_name = "NAME")]
    pub folder: Option<String>,
    /// Only this account
    #[arg(long, value_name = "EMAIL")]
    pub account: Option<String>,
    /// At most this many Threads, 50 when not given
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
}

#[derive(clap::Args, Deserialize, JsonSchema)]
pub struct MessageShow {
    /// The Message's ID, as a listing names it
    pub id: String,
}

#[derive(clap::Args, Deserialize, JsonSchema)]
pub struct MessageSearch {
    /// Words each matching the start of a word of the sender, subject,
    /// Snippet or body, and operators: from:, to:, subject:, is:unread,
    /// has:attachment, in:, account:, before:, after:, older:, newer:, -word
    #[arg(allow_hyphen_values = true)]
    pub query: String,
    /// At most this many Threads, 50 when not given
    #[arg(long, value_name = "N")]
    pub limit: Option<usize>,
}

#[derive(clap::Args, Deserialize, JsonSchema)]
pub struct CorrespondentList {
    /// Rank first the account this Draft goes out from
    #[arg(long, value_name = "PATH", conflicts_with = "account")]
    pub draft: Option<PathBuf>,
    /// Rank first this account
    #[arg(long, value_name = "EMAIL")]
    pub account: Option<String>,
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

    let matches = Args::command()
        .before_help(super::lockup::lockup())
        .after_help(after)
        .get_matches();
    Args::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
}
