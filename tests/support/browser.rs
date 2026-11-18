use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{Duration, Instant},
};

use tempfile::TempDir;

/// A directory ahead on `PATH` whose `open` and `xdg-open` write each URL
/// they are given to a file.
pub struct Browser {
    dir: TempDir,
}

impl Browser {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\n",
            dir.path().join("opened").display()
        );
        for name in ["open", "xdg-open"] {
            let path = dir.path().join(name);
            fs::write(&path, &script).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        Self { dir }
    }

    pub fn path(&self) -> PathBuf {
        let rest = std::env::var_os("PATH").unwrap();
        let mut paths = vec![self.dir.path().to_owned()];
        paths.extend(std::env::split_paths(&rest));
        std::env::join_paths(paths).unwrap().into()
    }

    pub fn opened(&self) -> Vec<String> {
        fs::read_to_string(self.dir.path().join("opened"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Waits for `urls` to be every URL opened so far.
    pub fn wait_for(&self, urls: &[&str]) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.opened() != urls {
            assert!(Instant::now() < deadline, "opened {:?}", self.opened());
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
