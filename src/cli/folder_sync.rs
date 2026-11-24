use std::{error::Error, fmt::Write, path::Path, time::Duration};

use serde::Serialize;

use crate::{
    account::config::Config,
    account::sign_in::SignIn,
    account::xdg,
    cache::{self, Cache, Reader},
    cli::accounts,
    cli::answer::{Answer, Answers},
    cli::args::FolderSync,
    cli::named_folder::named,
    compose::outbox::Outbox,
    compose::outgoing::Outgoing,
    domain::folder::INBOX,
    domain::folder_name::FolderName,
    domain::modified_utf7,
    domain::per_account::{AccountId, PerAccount},
    mail::folder_sync,
    mail::outbox_flush::Sender,
    mail::sync_lock::SyncLock,
};

/// How long one Account's sync may take before it counts as failed: no
/// IMAP command times out on its own, and a TUI waits on the lock meanwhile.
const PATIENCE: Duration = Duration::from_secs(5 * 60);

/// One Folder synced, or one Account not, in `--json`, whose fields only
/// grow (ADR 0017).
#[derive(Serialize)]
struct Row {
    account: String,
    /// Decoded from modified UTF-7; `null` for an Account not synced.
    folder: Option<String>,
    /// `null` for an Account not synced.
    new: Option<usize>,
    outcome: Outcome,
    /// What failed, for `failed`.
    error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Outcome {
    Synced,
    /// A TUI is open and keeps the Cache live itself.
    Open,
    SignedOut,
    Failed,
}

impl Row {
    fn unsynced(account: &str, outcome: Outcome, error: Option<String>) -> Self {
        Self {
            account: account.to_owned(),
            folder: None,
            new: None,
            outcome,
            error,
        }
    }
}

/// `postal folder sync`: what the TUI left queued sent, then each Folder
/// brought up to the server's; a TUI open keeps the Cache live, so it
/// steps aside for it (ADR 0033).
impl Answers for FolderSync {
    fn answer(&self) -> Result<Answer, Box<dyn Error>> {
        // The MCP server answers on its own runtime, which cannot block on another.
        let rows = std::thread::scope(|scope| {
            scope
                .spawn(|| self.rows().map_err(|error| error.to_string()))
                .join()
        })
        .map_err(|_panic| "the sync panicked")??;
        let mut answer = Answer::new(&rows, table(&rows))?;
        answer.failed = rows
            .iter()
            .any(|row| matches!(row.outcome, Outcome::SignedOut | Outcome::Failed));
        Ok(answer)
    }
}

impl FolderSync {
    fn rows(&self) -> Result<Vec<Row>, Box<dyn Error>> {
        let wanted = accounts::named(self.account.as_deref())?;
        let dir = xdg::cache_dir()?;
        let Some(_lock) = SyncLock::try_take(&dir)? else {
            return Ok(wanted
                .iter()
                .map(|account| Row::unsynced(account, Outcome::Open, None))
                .collect());
        };

        let config_dir = xdg::config_dir()?;
        let configs: PerAccount<Config> = Config::load(&config_dir.join("config.toml"))?
            .into_iter()
            .collect();
        // Every Account's handle, as the TUI opens it, though only some sync.
        let caches = Cache::open(&dir, &configs.map(|config| config.email.as_str()))?;
        let places = self.places(&dir, &configs, &wanted)?;
        let outbox = Outbox::new(xdg::data_dir().ok());
        let accounts = Accounts {
            configs: &configs,
            config_dir: &config_dir,
            caches: &caches,
            outbox: &outbox,
        };

        let work = (places.into_iter()).map(|(account, folder)| accounts.sync(account, folder));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let rows = runtime.block_on(futures::future::join_all(work));
        cache::settle(&dir);
        Ok(rows)
    }

    /// The Folders each wanted Account syncs: its Inbox, else the one
    /// `--folder` names, which only the Folders a TUI listed can answer.
    fn places(
        &self,
        dir: &Path,
        configs: &PerAccount<Config>,
        wanted: &[String],
    ) -> Result<Vec<(AccountId, FolderName)>, Box<dyn Error>> {
        let folders = match &self.folder {
            Some(_) => Reader::open(dir)?.folders()?,
            None => Vec::new(),
        };

        let mut places = Vec::new();
        for (account, config) in configs.iter() {
            let email = config.email.to_lowercase();
            if !wanted.contains(&email) {
                continue;
            }

            let name = (self.folder.as_ref())
                .map_or(Some(INBOX), |folder| named(&folders, &email, folder));
            places.extend(name.map(|name| (account, FolderName::from(name))));
        }

        if places.is_empty()
            && let Some(folder) = &self.folder
        {
            return Err(format!("no Account has a Folder {folder}").into());
        }

        Ok(places)
    }
}

/// What a sync of each Account takes.
struct Accounts<'a> {
    configs: &'a PerAccount<Config>,
    config_dir: &'a Path,
    caches: &'a PerAccount<Cache>,
    outbox: &'a Outbox,
}

impl Accounts<'_> {
    async fn sync(&self, account: AccountId, folder: FolderName) -> Row {
        let config = &self.configs[account];
        let email = config.email.to_lowercase();
        let sign_in = match SignIn::stored(config, self.config_dir) {
            Ok(Some(sign_in)) => sign_in,
            Ok(None) => return Row::unsynced(&email, Outcome::SignedOut, None),
            Err(error) => {
                return Row::unsynced(&email, Outcome::Failed, Some(error.to_string()));
            }
        };

        let sender = Sender {
            outbox: self.outbox,
            outgoing: Outgoing { config },
            sign_in: &sign_in,
            first: self.configs.first() == Some(account),
        };

        let synced = folder_sync::sync(&self.caches[account], sender, &folder);
        match tokio::time::timeout(PATIENCE, synced).await {
            Ok(Ok(new)) => Row {
                account: email,
                folder: Some(modified_utf7::decode(folder.as_str()).into_owned()),
                new: Some(new),
                outcome: Outcome::Synced,
                error: None,
            },
            Ok(Err(error)) if error.signed_out() => Row::unsynced(&email, Outcome::SignedOut, None),
            Ok(Err(error)) => Row::unsynced(&email, Outcome::Failed, Some(error.to_string())),
            Err(_elapsed) => {
                let error = format!("timed out after {} s", PATIENCE.as_secs());
                Row::unsynced(&email, Outcome::Failed, Some(error))
            }
        }
    }
}

/// A line each: the Account and Folder with its new Messages, or what kept
/// the Account from syncing.
fn table(rows: &[Row]) -> String {
    let mut text = String::new();
    // Writing to a `String` cannot fail.
    for row in rows {
        match (row.outcome, &row.folder, row.new) {
            (Outcome::Synced, Some(folder), Some(new)) => {
                writeln!(text, "{} {folder}: {new} new", row.account).ok();
            }
            (Outcome::Open, ..) => {
                writeln!(text, "{}: Postal is open and keeps it live", row.account).ok();
            }
            (Outcome::SignedOut, ..) => {
                writeln!(text, "{}: signed out; open Postal to sign in", row.account).ok();
            }
            _ => {
                let error = row.error.as_deref().unwrap_or("failed");
                writeln!(text, "{}: {error}", row.account).ok();
            }
        }
    }

    text
}
