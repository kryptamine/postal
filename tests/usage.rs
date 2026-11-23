#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::process::Command;

use support::home::Home;

const LOCKUP: &str = include_str!("../assets/brand/postal-lockup.txt");

fn help(args: &[&str]) -> String {
    let home = Home::new("");
    let output = Command::new(env!("CARGO_BIN_EXE_postal"))
        .args(args)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

// The lockup greets the user once, above the top-level help, and stays
// out of each command's own help, which a user reads for one flag.
#[test]
fn top_level_help_opens_with_the_lockup_alone() {
    assert!(help(&["--help"]).starts_with(&format!("{LOCKUP}\n\n")));
    assert!(!help(&["message", "--help"]).contains(LOCKUP));
}
