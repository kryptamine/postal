use crate::support;

use support::home::Home;

// A user reporting a bug names the release they run, without an account set up.
#[test]
fn version_prints_the_release_without_a_config() {
    let home = Home::new("");
    let output = home
        .command(env!("CARGO_BIN_EXE_postal"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, format!("postal {}\n", env!("CARGO_PKG_VERSION")));
}
