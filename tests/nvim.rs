//! The Neovim plugin's smoke check, `nvim/scripts/smoke.lua`, passes with
//! the real binary over a Cache the TUI synced, not only with the stub
//! `make smoke` gives it: the CLI's answer is what the plugin reads. Needs
//! `nvim`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{path::Path, process::Command, time::Duration};

use support::{
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
};
use termlens::{Key, Terminal};

fn synced(imap: &FakeImap, smtp: &FakeSmtp) -> Home {
    let server = |port: u16| format!("host = \"127.0.0.1\"\nport = {port}\ntls = false\n");
    let home = Home::new(&format!(
        "email = \"{EMAIL}\"\n\n[imap]\n{}\n[smtp]\n{}\n[secrets]\nstore = \"file\"\n{}",
        server(imap.port),
        server(smtp.port),
        fake_oauth::shared().config()
    ));
    let mut t = Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("TZ", "UTC")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap();
    t.wait_until(|s| s.contains("Notes")).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    home
}

fn message(name: &'static str, address: &'static str, date: &str) -> FakeMessage {
    FakeMessage {
        from_address: address,
        ..FakeMessage::new(name, "Notes", date)
    }
}

#[test]
fn the_plugin_completes_from_what_the_cli_lists() {
    let imap = FakeImap::start(vec![
        message("Petr Novak", "petr@example.com", "2024-11-18T09:00:00Z"),
        message("Peter Pan", "pan@example.com", "2024-11-17T09:00:00Z"),
        message("Ada Lovelace", "ada@example.com", "2024-11-16T09:00:00Z"),
    ]);
    let smtp = FakeSmtp::start();
    let home = synced(&imap, &smtp);
    let plugin = Path::new(env!("CARGO_MANIFEST_DIR")).join("nvim");
    let output = Command::new("nvim")
        .args(["--clean", "--headless", "--cmd", "set rtp^=..", "-c"])
        .arg("lua vim.schedule(function() dofile('scripts/smoke.lua') end)")
        .current_dir(plugin)
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("POSTAL_BIN", env!("CARGO_BIN_EXE_postal"))
        .output()
        .expect("nvim on PATH");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("smoke: ok"));
}
