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

/// The TUI with `$EDITOR` a headless Neovim running the plugin, which sets
/// the Draft's `To` and `Subject`, runs `:PostalSend`, then writes and quits.
/// Its stdin is no terminal, where an error would wait for `Enter`.
fn sent_from_neovim(home: &Home, to: &str, subject: &str) -> Terminal {
    let repo = env!("CARGO_MANIFEST_DIR");
    let bin = env!("CARGO_BIN_EXE_postal");
    let editor = format!(
        "nvim --clean --headless --cmd 'set rtp^={repo}' \
         --cmd 'lua require(\"postal\").setup({{ cmd = \"{bin}\" }})' \
         -c '1s/$/ {to}/' -c '3s/$/ {subject}/' -c 'PostalSend' -c 'wqall!' </dev/null"
    );
    Terminal::builder()
        .size(100, 30)
        .env_clear()
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("EDITOR", editor)
        .env("TZ", "UTC")
        .env("POSTAL_SEND_DELAY", "0")
        .timeout(Duration::from_secs(10))
        .spawn(env!("CARGO_BIN_EXE_postal"))
        .unwrap()
}

#[test]
fn postal_send_in_neovim_sends_the_draft_with_no_preview() {
    let imap = FakeImap::start(vec![message(
        "Ada",
        "ada@example.com",
        "2024-11-18T09:00:00Z",
    )]);
    let smtp = FakeSmtp::start();
    let home = synced(&imap, &smtp);
    let mut t = sent_from_neovim(&home, "bob@example.com", "Lunch");
    t.wait_until(|s| s.contains("Notes")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains("Sent") && !s.contains("y send"))
        .unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());

    let received = smtp.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].rcpt_to, ["bob@example.com"]);
}

// A Draft the checks find wrong stays in the editor, unsent; written and
// quit, it is kept like any other.
#[test]
fn postal_send_refuses_a_draft_with_a_bad_address() {
    let imap = FakeImap::start(vec![message(
        "Ada",
        "ada@example.com",
        "2024-11-18T09:00:00Z",
    )]);
    let smtp = FakeSmtp::start();
    let home = synced(&imap, &smtp);
    let mut t = sent_from_neovim(&home, "bob at example", "Lunch");
    t.wait_until(|s| s.contains("Notes")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains("Saved to Drafts")).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert!(smtp.received().is_empty());
}

// A warning holds the Send back as an error does, until `:PostalSend!`.
#[test]
fn postal_send_holds_back_a_draft_with_no_subject() {
    let imap = FakeImap::start(vec![message(
        "Ada",
        "ada@example.com",
        "2024-11-18T09:00:00Z",
    )]);
    let smtp = FakeSmtp::start();
    let home = synced(&imap, &smtp);
    let mut t = sent_from_neovim(&home, "bob@example.com", "");
    t.wait_until(|s| s.contains("Notes")).unwrap();
    t.send(Key::Char('c')).unwrap();
    t.wait_until(|s| s.contains("Saved to Drafts")).unwrap();
    t.send(Key::Ctrl('c')).unwrap();
    assert!(t.wait_exit().unwrap().success());
    assert!(smtp.received().is_empty());
}
