//! `make demo`: Postal against the fake servers of the tests, seeded with
//! the demo mailbox, in a throwaway config, Cache and Neovim. Every asset of
//! the landing page is shot from it; see `demo/README.md`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a demo that cannot start has nothing to show"
)]

// The part of the tests' harness that serves mail; the rest of it drives a
// terminal under test, which `env!("CARGO_BIN_EXE_postal")` ties to tests.
#[path = "../tests/support"]
mod support {
    #![allow(dead_code, reason = "the demo uses part of the harness")]

    pub mod fake_body;
    pub mod fake_imap;
    pub mod fake_oauth;
    pub mod fake_smtp;
    pub mod home;
    mod query;
    mod relay;
}

mod home;
mod mailbox;

use std::{path::PathBuf, process::Command};

use support::{fake_imap::FakeImap, fake_smtp::FakeSmtp};

fn main() {
    let mut args = std::env::args_os().skip(1).peekable();
    let postal = PathBuf::from(args.next().expect("the path of the postal binary"));
    let onboarding = args.next_if(|arg| arg == "--onboarding").is_some();
    let work = FakeImap::start(Vec::new());
    mailbox::work(&work);
    let personal = FakeImap::start(Vec::new());
    mailbox::personal(&personal);
    let smtp = (FakeSmtp::start(), FakeSmtp::start());
    let home = home::Home::new(&postal, (&work, &personal), (&smtp.0, &smtp.1));
    if onboarding {
        home.forget_accounts();
    }

    let status = Command::new(&postal)
        .args(args)
        .envs(home.env())
        .status()
        .expect("postal starts");
    drop(home);
    std::process::exit(status.code().unwrap_or(1));
}
