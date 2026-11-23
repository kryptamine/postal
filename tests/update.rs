#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

use support::fake_releases::{FakeReleases, archive_path, checksum_path};
use tempfile::TempDir;

const RUNNING: &str = env!("CARGO_PKG_VERSION");
const NEW: &str = "#!/bin/sh\necho 'postal 99.0.0'\n";

/// A copy of postal at `bin` under a throwaway `$HOME`: `bin` as the
/// install script leaves it, or `.cargo/bin` as `make install` does.
struct Installed {
    home: TempDir,
    bin: &'static str,
}

impl Installed {
    fn at(bin: &'static str) -> Self {
        let installed = Self {
            home: tempfile::tempdir().unwrap(),
            bin,
        };

        fs::create_dir_all(installed.bin_dir()).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_postal"), installed.binary()).unwrap();
        installed
    }

    fn bin_dir(&self) -> PathBuf {
        self.home.path().join(self.bin)
    }

    fn binary(&self) -> PathBuf {
        self.bin_dir().join("postal")
    }

    fn update(&self, releases: &FakeReleases) -> Output {
        Command::new(self.binary())
            .arg("update")
            .env_clear()
            .env("HOME", self.home.path())
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("POSTAL_RELEASES_URL", releases.url())
            .output()
            .unwrap()
    }

    fn version(&self) -> String {
        let output = Command::new(self.binary())
            .arg("--version")
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap()
    }

    /// The names in the binary's directory, so a test sees what an update left behind.
    fn left(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.bin_dir())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    /// Asserts the update failed saying `why`, and left the binary as it was.
    fn assert_kept(&self, output: &Output, why: &str) {
        assert!(!output.status.success(), "{output:?}");
        let stderr = String::from_utf8(output.stderr.clone()).unwrap();
        assert!(stderr.contains(why), "{output:?}");
        assert_eq!(self.version(), format!("postal {RUNNING}\n"));
        assert_eq!(self.left(), ["postal"]);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

// A user of the install script gets the latest release in place, without the script.
#[test]
fn update_replaces_the_binary_with_the_latest_release() {
    let releases = FakeReleases::start();
    releases.publish("v99.0.0", NEW);
    let installed = Installed::at("bin");

    let output = installed.update(&releases);

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        stdout(&output),
        format!("postal {RUNNING} updated to 99.0.0\n")
    );
    assert_eq!(installed.version(), "postal 99.0.0\n");
    assert_eq!(installed.left(), ["postal"]);
    assert_eq!(
        releases.paths(),
        [
            "/releases/latest".to_owned(),
            archive_path("v99.0.0"),
            checksum_path("v99.0.0"),
        ]
    );
}

// Running update on the latest release downloads nothing and keeps the binary.
#[test]
fn update_on_the_latest_release_changes_nothing() {
    let releases = FakeReleases::start();
    releases.publish_tag(&format!("v{RUNNING}"));
    let installed = Installed::at("bin");

    let output = installed.update(&releases);

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        stdout(&output),
        format!("postal {RUNNING} is the latest release\n")
    );
    assert_eq!(installed.version(), format!("postal {RUNNING}\n"));
    assert_eq!(releases.paths(), ["/releases/latest"]);
}

// A release with no archive for this target sends the user to a build from source.
#[test]
fn update_without_an_archive_for_this_target_keeps_the_binary() {
    let releases = FakeReleases::start();
    releases.publish_tag("v99.0.0");
    let installed = Installed::at("bin");

    let output = installed.update(&releases);

    installed.assert_kept(&output, "build from source");
}

// An archive altered on the way never replaces the binary that works.
#[test]
fn update_refuses_an_archive_that_fails_its_checksum() {
    let releases = FakeReleases::start();
    releases.publish("v99.0.0", NEW);
    releases.break_checksum("v99.0.0");
    let installed = Installed::at("bin");

    let output = installed.update(&releases);

    installed.assert_kept(&output, "does not match its sha256");
}

// A release built for another machine, or broken, is caught before the swap.
#[test]
fn update_keeps_the_binary_when_the_release_does_not_run() {
    let releases = FakeReleases::start();
    releases.publish("v99.0.0", "#!/bin/sh\nexit 1\n");
    let installed = Installed::at("bin");

    let output = installed.update(&releases);

    installed.assert_kept(&output, "does not run here");
}

// A build from source belongs to cargo: replacing it would hide the checkout's changes.
#[test]
fn update_leaves_a_cargo_install_to_cargo() {
    let releases = FakeReleases::start();
    releases.publish("v99.0.0", NEW);
    let installed = Installed::at(".cargo/bin");

    let output = installed.update(&releases);

    installed.assert_kept(&output, "git pull && make install");
    assert_eq!(releases.paths(), Vec::<String>::new());
}
