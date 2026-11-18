//! A throwaway `$XDG_CONFIG_HOME` holding Postal's config and secret, and an
//! `$XDG_STATE_HOME` for its log and an `$XDG_CACHE_HOME` for its Cache.

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

use tempfile::TempDir;

use super::fake_imap::{EMAIL, FakeImap, PASSWORD};

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
        let secret = home.postal_dir().join("secret");
        fs::write(&secret, format!("{PASSWORD}\n")).unwrap();
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
        home
    }

    /// A valid config pointing at `server` over plaintext loopback.
    pub fn for_server(server: &FakeImap) -> Self {
        Self::new(&format!(
            "email = \"{EMAIL}\"\nauth = \"app-password\"\n\n[imap]\nhost = \"127.0.0.1\"\nport = {}\ntls = false\n\n[secrets]\nstore = \"file\"\n",
            server.port
        ))
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
        self.postal_dir().join("secret")
    }

    fn postal_dir(&self) -> PathBuf {
        self.dir.path().join("postal")
    }
}
