#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::process::Command;

use support::home::Home;

// A user reporting a bug names the release they run, without an account set up.
#[test]
fn version_prints_the_release_without_a_config() {
    let home = Home::new("");
    let output = Command::new(env!("CARGO_BIN_EXE_postal"))
        .arg("--version")
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, format!("postal {}\n", env!("CARGO_PKG_VERSION")));
}
