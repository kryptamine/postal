#![allow(
    dead_code,
    unused_imports,
    reason = "each test binary uses part of the harness"
)]

pub mod browser;
pub mod client_config;
pub mod editor;
pub mod fake_autoconfig;
pub mod fake_body;
pub mod fake_dns;
pub mod fake_http;
pub mod fake_imap;
pub mod fake_oauth;
pub mod fake_releases;
pub mod fake_smtp;
pub mod home;
pub mod notifier;
pub mod pager;
pub mod picker;
pub mod query;
mod quit;
mod relay;
pub mod rendered;
pub mod search;
mod wait;

pub use quit::quit;
pub use wait::wait_for;
