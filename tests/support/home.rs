//! A throwaway `$XDG_CONFIG_HOME` holding Postal's config and secret, an
//! `$XDG_STATE_HOME` for its log, an `$XDG_CACHE_HOME` for its Cache and an
//! `$XDG_DATA_HOME` for its Drafts and its Outbox.

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command, time::Duration};

use tempfile::TempDir;
use termlens::{Terminal, TerminalBuilder};

use super::{
    fake_imap::{EMAIL, FakeImap},
    fake_oauth::{self, REFRESH_TOKEN},
};

pub struct Home {
    dir: TempDir,
}

impl Home {
    pub fn new(config: &str) -> Self {
        let home = Self {
            dir: tempfile::tempdir().unwrap(),
        };

        fs::create_dir_all(home.postal_dir()).unwrap();
        fs::write(home.config_path(), config).unwrap();
        let secret = home.secret_path();
        fs::create_dir_all(secret.parent().unwrap()).unwrap();
        fs::write(&secret, format!("{REFRESH_TOKEN}\n")).unwrap();
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
        home
    }

    /// A valid config pointing at `server` over plaintext loopback, signed in
    /// through the shared fake token endpoint.
    pub fn for_server(server: &FakeImap) -> Self {
        Self::new(&format!(
            "email = \"{EMAIL}\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n{}",
            server.port,
            fake_oauth::shared().config()
        ))
    }

    /// An 80×24 terminal on this home alone, but `PATH`, for `sh` to find
    /// the commands of a `$PAGER` or `$EDITOR`.
    pub fn terminal(&self) -> TerminalBuilder {
        Terminal::builder()
            .size(80, 24)
            .env_clear()
            .env("XDG_CONFIG_HOME", self.xdg_config_home())
            .env("XDG_STATE_HOME", self.xdg_state_home())
            .env("XDG_CACHE_HOME", self.xdg_cache_home())
            .env("XDG_DATA_HOME", self.xdg_data_home())
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("TZ", "UTC")
            // A Send leaves at once; the hold has its own tests.
            .env("POSTAL_SEND_DELAY", "0")
            .timeout(Duration::from_secs(10))
    }

    /// `postal`, the binary a test names, as a command beside the TUI, on
    /// this home alone.
    pub fn command(&self, postal: &str) -> Command {
        let mut command = Command::new(postal);
        command
            .env_clear()
            .env("XDG_CONFIG_HOME", self.xdg_config_home())
            .env("XDG_STATE_HOME", self.xdg_state_home())
            .env("XDG_CACHE_HOME", self.xdg_cache_home())
            .env("XDG_DATA_HOME", self.xdg_data_home())
            .env("TZ", "UTC");
        command
    }

    pub fn xdg_config_home(&self) -> PathBuf {
        self.dir.path().to_owned()
    }

    pub fn xdg_state_home(&self) -> PathBuf {
        self.dir.path().join("state")
    }

    pub fn xdg_cache_home(&self) -> PathBuf {
        self.dir.path().join("cache")
    }

    pub fn xdg_data_home(&self) -> PathBuf {
        self.dir.path().join("data")
    }

    pub fn drafts_dir(&self) -> PathBuf {
        self.xdg_data_home().join("postal/drafts")
    }

    pub fn outbox_dir(&self) -> PathBuf {
        self.xdg_data_home().join("postal/outbox")
    }

    pub fn cache_path(&self) -> PathBuf {
        self.xdg_cache_home().join("postal/cache.sqlite3")
    }

    pub fn log_path(&self) -> PathBuf {
        self.xdg_state_home().join("postal/postal.log")
    }

    pub fn config_path(&self) -> PathBuf {
        self.postal_dir().join("config.toml")
    }

    pub fn secret_path(&self) -> PathBuf {
        self.secret_path_of(EMAIL)
    }

    /// The file store's secret of the Account `email`.
    pub fn secret_path_of(&self, email: &str) -> PathBuf {
        self.postal_dir().join("secrets").join(email.to_lowercase())
    }

    fn postal_dir(&self) -> PathBuf {
        self.dir.path().join("postal")
    }
}
