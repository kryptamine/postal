use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{Duration, Instant},
};

use tempfile::TempDir;

/// A directory ahead on `PATH` whose `osascript` and `notify-send` write
/// the last two arguments they are given, the title and the body, as one
/// line to a file.
pub struct Notifier {
    dir: TempDir,
}

impl Notifier {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let script = format!(
            "#!/bin/sh\nwhile [ $# -gt 2 ]; do shift; done\nprintf '%s | %s\\n' \"$1\" \"$2\" >> '{}'\n",
            dir.path().join("notified").display()
        );
        for name in ["osascript", "notify-send"] {
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

    /// Each notification so far, as `title | body`.
    pub fn notified(&self) -> Vec<String> {
        fs::read_to_string(self.dir.path().join("notified"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// Waits for `count` notifications, and returns them.
    pub fn wait_for(&self, count: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.notified().len() < count {
            assert!(Instant::now() < deadline, "notified {:?}", self.notified());
            std::thread::sleep(Duration::from_millis(10));
        }

        self.notified()
    }
}
