//! The integration tests as one binary, so `support` compiles and links once
//! rather than once per file. `autotests` is off: a new file is declared here.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test fails by panicking"
)]

mod support;

mod accounts;
mod architecture;
mod attach;
mod attachments;
mod avatars;
mod backfill;
mod body_index;
mod browser_view;
mod cache;
mod compose;
mod config;
mod contacts;
mod correspondent_list;
mod delete;
mod drafts;
mod editor_view;
mod folder_sync;
mod folders;
mod forward;
mod help;
mod html_preview;
mod icloud;
mod icons;
mod idle;
mod inbox;
mod layout_text;
mod links;
mod log;
mod login;
mod mail_commands;
mod mail_corpus;
mod mailto;
mod mark_read;
mod markdown;
mod mcp;
mod migrations;
mod r#move;
mod notify;
mod nvim;
mod other_provider;
mod outbox;
mod pager;
mod picks;
mod preview;
mod quotes;
mod reading;
mod reconnect;
mod rendered_buttons;
mod rendered_fit;
mod rendered_frames;
mod rendered_images;
mod rendered_text;
mod rendered_view;
mod reply;
mod search;
mod search_operators;
mod secrets;
mod settings;
mod setup_mailto;
mod sign_in;
mod signature;
mod snippets;
mod takeover;
mod thread_elsewhere;
mod thread_show;
mod threads;
mod update;
mod usage;
mod version;
mod window;
