use std::{borrow::Cow, error::Error, path::Path, time::Duration};

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
    mail::sync_lock::{Kind, SyncLock},
};

/// How long one Account's sync may take before it counts as failed: no
/// IMAP command times out on its own, and a TUI waits on the lock meanwhile.
const PATIENCE: Duration = Duration::from_secs(5 * 60);

/// How one Account's sync ended.
enum Outcome {
    Synced {
        folder: FolderName,
        new: usize,
    },
    /// A TUI is open and keeps the Cache live itself.
    Open,
    SignedOut,
    Failed(String),
}

/// One Account of `--json`, whose fields only grow (ADR 0017).
#[derive(Serialize)]
struct Row<'a> {
    account: &'a str,
    /// Decoded from modified UTF-7; `null` unless synced.
    folder: Option<Cow<'a, str>>,
    /// `null` unless synced.
    new: Option<usize>,
    /// `synced`, `open`, `signed-out` or `failed`.
    outcome: &'static str,
    /// What failed, for `failed`.
    error: Option<&'a str>,
}

impl Outcome {
    const fn failed(&self) -> bool {
        matches!(self, Self::SignedOut | Self::Failed(_))
    }

    fn row<'a>(&'a self, account: &'a str) -> Row<'a> {
        let (outcome, folder, new, error) = match self {
            Self::Synced { folder, new } => {
                let folder = modified_utf7::decode(folder.as_str());
                ("synced", Some(folder), Some(*new), None)
            }
            Self::Open => ("open", None, None, None),
            Self::SignedOut => ("signed-out", None, None, None),
            Self::Failed(error) => ("failed", None, None, Some(error.as_str())),
        };

        Row {
            account,
            folder,
            new,
            outcome,
            error,
        }
    }

    fn line(&self, account: &str) -> String {
        match self {
            Self::Synced { folder, new } => {
                let folder = modified_utf7::decode(folder.as_str());
                format!("{account} {folder}: {new} new\n")
            }
            Self::Open => format!("{account}: Postal is open and keeps it live\n"),
            Self::SignedOut => format!("{account}: signed out; open Postal to sign in\n"),
            Self::Failed(error) => format!("{account}: {error}\n"),
        }
    }
}

/// `postal folder sync`: what the TUI left queued sent, then each Folder
/// brought up to the server's; a TUI open keeps the Cache live, so it
/// steps aside for it (ADR 0033).
impl Answers for FolderSync {
    fn answer(&self) -> Result<Answer, Box<dyn Error>> {
        let synced = self.sync()?;
        let rows: Vec<_> = (synced.iter())
            .map(|(account, outcome)| outcome.row(account))
            .collect();
        let text = (synced.iter())
            .map(|(account, outcome)| outcome.line(account))
            .collect();
        let mut answer = Answer::new(&rows, text)?;
        answer.failed = synced.iter().any(|(_, outcome)| outcome.failed());
        Ok(answer)
    }
}

impl FolderSync {
    /// Each wanted Account by its address, with how its sync ended.
    fn sync(&self) -> Result<Vec<(String, Outcome)>, Box<dyn Error>> {
        let wanted = accounts::named(self.account.as_deref())?;
        let dir = xdg::cache_dir()?;
        let Some(_lock) = SyncLock::try_take(Kind::Sync)? else {
            return Ok(wanted.into_iter().map(|a| (a, Outcome::Open)).collect());
        };

        let config_dir = xdg::config_dir()?;
        let configs: PerAccount<Config> = Config::load(&config_dir.join("config.toml"))?
            .into_iter()
            .collect();
        // Every Account's handle, as the TUI opens it, though only some sync.
        let caches = Cache::open(&dir, &configs.map(|config| config.email.as_str()))?;
        let places = self.places(&dir, &configs, &wanted)?;
        let outbox = Outbox::new(xdg::data_dir().ok());
        let syncing = Syncing {
            configs: &configs,
            config_dir: &config_dir,
            caches: &caches,
            outbox: &outbox,
        };

        let work = (places.into_iter()).map(|(account, folder)| async move {
            let email = syncing.configs[account].email.to_lowercase();
            (email, syncing.account(account, folder).await)
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let synced = runtime.block_on(futures::future::join_all(work));
        cache::settle(&dir);
        Ok(synced)
    }

    /// The Folder each wanted Account syncs: its Inbox, else the one
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
#[derive(Clone, Copy)]
struct Syncing<'a> {
    configs: &'a PerAccount<Config>,
    config_dir: &'a Path,
    caches: &'a PerAccount<Cache>,
    outbox: &'a Outbox,
}

impl Syncing<'_> {
    async fn account(self, account: AccountId, folder: FolderName) -> Outcome {
        let config = &self.configs[account];
        let sign_in = match SignIn::stored(config, self.config_dir) {
            Ok(Some(sign_in)) => sign_in,
            Ok(None) => return Outcome::SignedOut,
            Err(error) => return Outcome::Failed(error.to_string()),
        };

        let sender = Sender {
            outbox: self.outbox,
            outgoing: Outgoing { config },
            sign_in: &sign_in,
            first: self.configs.first() == Some(account),
        };

        let synced = folder_sync::sync(&self.caches[account], sender, &folder);
        match tokio::time::timeout(PATIENCE, synced).await {
            Ok(Ok(new)) => Outcome::Synced { folder, new },
            Ok(Err(error)) if error.signed_out() => Outcome::SignedOut,
            Ok(Err(error)) => Outcome::Failed(error.to_string()),
            Err(_elapsed) => Outcome::Failed(format!("timed out after {} s", PATIENCE.as_secs())),
        }
    }
}
