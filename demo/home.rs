//! The throwaway home of a demo run: two Accounts over the fake servers,
//! and a Neovim that loads the plugin of this checkout and nothing of the
//! user's own.

use std::{
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::support::{
    fake_imap::{APP_PASSWORD, EMAIL, FakeImap},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home as Dirs,
};

/// The second Account, at an iCloud-like server.
pub const PERSONAL: &str = "sam@icloud.example";

pub struct Home {
    dirs: Dirs,
    postal: PathBuf,
}

impl Home {
    pub fn new(postal: &Path, imap: (&FakeImap, &FakeImap), smtp: (&FakeSmtp, &FakeSmtp)) -> Self {
        let server = |port: u16| format!("host = \"127.0.0.1\"\nport = {port}\ntls = false\n");
        let dirs = Dirs::new(&format!(
            "email = \"{EMAIL}\"\nname = \"Sam Okafor\"\nicons = \"nerd\"\n\n\
             [imap]\n{}\n[smtp]\n{}\n[secrets]\nstore = \"file\"\n{}\n\
             [[account]]\nemail = \"{PERSONAL}\"\nname = \"Sam Okafor\"\nprovider = \"icloud\"\n\n\
             [account.imap]\n{}\n[account.smtp]\n{}",
            server(imap.0.port),
            server(smtp.0.port),
            fake_oauth::shared().config(),
            server(imap.1.port),
            server(smtp.1.port),
        ));
        let secret = dirs.secret_path_of(PERSONAL);
        fs::write(&secret, format!("{APP_PASSWORD}\n")).unwrap();
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
        let home = Self {
            dirs,
            postal: postal.canonicalize().unwrap(),
        };
        home.write_nvim();
        home.write_files();
        home
    }

    /// What the run sets over the user's environment: the terminal, its
    /// font and `PATH` stay, the rest of the user's is out of reach, their
    /// home too, which an `Attach:` line would otherwise complete from.
    pub fn env(&self) -> Vec<(OsString, OsString)> {
        let dirs = &self.dirs;
        [
            ("HOME", self.home_dir()),
            ("XDG_CONFIG_HOME", dirs.xdg_config_home()),
            ("XDG_STATE_HOME", dirs.xdg_state_home()),
            ("XDG_CACHE_HOME", dirs.xdg_cache_home()),
            ("XDG_DATA_HOME", dirs.xdg_data_home()),
        ]
        .into_iter()
        .map(|(name, path)| (name.into(), path.into_os_string()))
        .chain([
            ("EDITOR".into(), "nvim".into()),
            ("NVIM_APPNAME".into(), "nvim".into()),
        ])
        .collect()
    }

    /// Leaves no Account signed in, so Postal opens on the Onboarding.
    pub fn forget_accounts(&self) {
        fs::remove_file(self.dirs.config_path()).unwrap();
    }

    fn home_dir(&self) -> PathBuf {
        self.dirs.xdg_config_home().join("home")
    }

    /// A few files for an `Attach:` line to find.
    fn write_files(&self) {
        let home = self.home_dir();
        for file in [
            "Documents/porto-agenda.pdf",
            "Documents/q4-budget-notes.md",
            "Documents/search-demo-slides.pdf",
            "Pictures/ridge-walk.jpg",
        ] {
            let path = home.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "demo\n").unwrap();
        }
    }

    fn write_nvim(&self) {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = self.dirs.xdg_config_home().join("nvim");
        fs::create_dir_all(&dir).unwrap();
        let init = include_str!("init.lua")
            .replace("@PLUGIN@", &repo.join("nvim").display().to_string())
            .replace(
                "@AGENT@",
                &repo.join("demo/agent.lua").display().to_string(),
            )
            .replace("@POSTAL@", &self.postal.display().to_string());
        fs::write(dir.join("init.lua"), init).unwrap();
    }
}
