use std::path::Path;

use crate::{
    accounts::Accounts, cache::Cache, outbox::Outbox, per_account::PerAccount, routes::Routes,
    steering::Ask,
};

/// What the main screen acts on: each Account's Cache and sync connection,
/// the Outbox and the config.
#[derive(Clone, Copy)]
pub struct World<'a> {
    pub caches: &'a PerAccount<Cache>,
    pub asks: &'a Routes<Ask>,
    pub outbox: &'a Outbox,
    pub accounts: Accounts<'a>,
    /// Where `config.toml` is, which Settings writes.
    pub config_dir: &'a Path,
}
