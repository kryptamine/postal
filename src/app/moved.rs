use std::sync::OnceLock;

use tokio::signal::unix::{SignalKind, signal};

use crate::{account::xdg, cache, mail::handover, mail::held, ui::terminal_backend};

/// The terminal a takeover moved the session to, once SIGTERM is heard.
static TARGET: OnceLock<Option<String>> = OnceLock::new();

/// Hears SIGTERM, which a TUI in another terminal sends to take the
/// session over, on a thread of its own: an editor in the foreground
/// blocks the event loop's.
pub fn listen() {
    std::thread::spawn(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let Ok(runtime) = runtime else {
            return;
        };

        runtime.block_on(async {
            let Ok(mut terms) = signal(SignalKind::terminate()) else {
                tracing::warn!("listening for SIGTERM failed");
                return;
            };

            while terms.recv().await.is_some() {
                heard();
            }
        });
    });
}

#[expect(
    clippy::print_stderr,
    reason = "the screen is handed back, so stderr reaches the user"
)]
fn heard() {
    // Read before the lock goes: the new TUI then drops it.
    TARGET.get_or_init(handover::target);
    if held::serving() {
        held::leave();
        // The event loop waits on the program, and holds nothing half
        // done but a Send, which keeps the lock until it ends.
        if held::outside() && !held::sending() {
            if let Ok(dir) = xdg::cache_dir() {
                cache::settle(&dir);
            }

            held::release();
        }

        return;
    }

    // Between sessions, or before the first, nothing is half done.
    terminal_backend::restore();
    eprintln!("{}", line());
    std::process::exit(0);
}

/// What the TUI leaves in its terminal as it quits on SIGTERM.
pub fn line() -> String {
    TARGET.get().cloned().flatten().map_or_else(
        || "postal: quit on SIGTERM".to_owned(),
        |target| format!("postal moved to {target}"),
    )
}
