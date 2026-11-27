#[cfg(target_os = "macos")]
mod address_book;
pub mod atomic_file;
pub mod autoconfig;
pub mod browser;
pub mod config;
pub mod config_file;
pub mod contacts;
#[cfg(target_os = "macos")]
mod contacts_fake;
#[cfg(target_os = "macos")]
pub mod contacts_process;
pub mod credential;
#[cfg_attr(
    not(target_os = "macos"),
    expect(dead_code, reason = "compiled everywhere, so every system lints it")
)]
mod handler_app;
#[cfg_attr(
    target_os = "macos",
    expect(dead_code, reason = "compiled everywhere, so every system lints it")
)]
mod handler_desktop;
mod keychain;
pub mod mailto_handler;
pub mod named_file;
pub mod oauth;
pub mod partial_config;
pub mod provider;
mod removed;
pub mod run;
mod secret;
mod secret_store;
pub mod sign_in;
pub mod signature;
pub mod unlink;
mod written;
pub mod xdg;
