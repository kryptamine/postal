//! The Neovim plugin in `nvim/` completes a Draft's recipients in `To:` and
//! `Cc:` from `postal correspondent list`, run as the real binary over a
//! Cache the TUI synced, and offers nothing in the body. Needs `nvim`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

use std::{fs, path::Path, process::Command, time::Duration};

use serde_json::Value;
use support::{
    fake_imap::{EMAIL, FakeImap, FakeMessage},
    fake_oauth,
    fake_smtp::FakeSmtp,
    home::Home,
};
use termlens::{Key, Terminal};

/// Asks the plugin's `omnifunc` at two places of the Draft and writes what
/// it answered to `$OUT`.
const SCRIPT: &str = r#"
require("postal").setup({ cmd = os.getenv("POSTAL_BIN") })
vim.cmd.edit(os.getenv("DRAFT"))
local omnifunc = require("postal").omnifunc
local function at(row, col)
  vim.api.nvim_win_set_cursor(0, { row, col })
  local start = omnifunc(1, "")
  if start < 0 then
    return { start = start }
  end
  local base = vim.api.nvim_get_current_line():sub(start + 1, col)
  return { start = start, items = omnifunc(0, base) }
end
local answer = {
  omnifunc = vim.bo.omnifunc,
  cc = at(2, #"Cc: Ada Lovelace <ada@example.com>, pet"),
  body = at(5, #"pet"),
}
vim.fn.writefile({ vim.json.encode(answer) }, os.getenv("OUT"))
vim.cmd("qa!")
"#;

const DRAFT: &str =
    "To: bob@example.com\nCc: Ada Lovelace <ada@example.com>, pet\nSubject: Hi\n\npet\n";

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
fn a_recipient_is_completed_from_correspondents_in_the_header_alone() {
    let imap = FakeImap::start(vec![
        message("Petr Novak", "petr@example.com", "2024-11-18T09:00:00Z"),
        message("Peter Pan", "pan@example.com", "2024-11-17T09:00:00Z"),
        message("Ada Lovelace", "ada@example.com", "2024-11-16T09:00:00Z"),
    ]);
    let smtp = FakeSmtp::start();
    let home = synced(&imap, &smtp);
    let drafts = home.drafts_dir();
    fs::create_dir_all(&drafts).unwrap();
    let draft = drafts.join("1.eml");
    fs::write(&draft, DRAFT).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (script, out) = (dir.path().join("test.lua"), dir.path().join("out.json"));
    fs::write(&script, SCRIPT).unwrap();

    let plugin = Path::new(env!("CARGO_MANIFEST_DIR")).join("nvim");
    let status = Command::new("nvim")
        .args(["--headless", "--clean", "--cmd"])
        .arg(format!("set rtp^={}", plugin.display()))
        .arg("-c")
        .arg(format!("luafile {}", script.display()))
        .env("XDG_CONFIG_HOME", home.xdg_config_home())
        .env("XDG_STATE_HOME", home.xdg_state_home())
        .env("XDG_CACHE_HOME", home.xdg_cache_home())
        .env("XDG_DATA_HOME", home.xdg_data_home())
        .env("POSTAL_BIN", env!("CARGO_BIN_EXE_postal"))
        .env("DRAFT", &draft)
        .env("OUT", &out)
        .status()
        .expect("nvim on PATH");
    assert!(status.success());

    let answer: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(answer["omnifunc"], "v:lua.require'postal'.omnifunc");
    // The recipient after the comma, not the whole header.
    let cc = &answer["cc"];
    assert_eq!(cc["start"], "Cc: Ada Lovelace <ada@example.com>, ".len());
    let words: Vec<&str> = cc["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["word"].as_str().unwrap())
        .collect();
    assert_eq!(
        words,
        ["Petr Novak <petr@example.com>", "Peter Pan <pan@example.com>"]
    );
    assert_eq!(answer["body"]["start"], -3);
}
