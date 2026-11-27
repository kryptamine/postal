use std::{fs, io};

use crate::mail::takeover::sync_lock::{self, Holder};

/// Beside the lock: the terminal a takeover moves the session to.
const TARGET: &str = "postal.moving";

/// Asks `holder` to quit, naming this terminal for the line it leaves.
pub fn ask(holder: &Holder) -> io::Result<()> {
    let tty = sync_lock::tty().unwrap_or_else(|| "another terminal".to_owned());
    fs::write(sync_lock::dir()?.join(TARGET), tty)?;
    signal(holder, libc::SIGTERM)
}

/// Kills `holder`, which did not answer `ask`.
pub fn force(holder: &Holder) -> io::Result<()> {
    signal(holder, libc::SIGKILL)
}

/// The terminal the session moved to, taken from beside the lock; `None`
/// for a SIGTERM no takeover sent.
pub fn target() -> Option<String> {
    let path = sync_lock::dir().ok()?.join(TARGET);
    let target = fs::read_to_string(&path).ok()?;
    fs::remove_file(&path).ok();
    Some(target)
}

/// Drops the terminal a holder forced out never read, so a later SIGTERM
/// from elsewhere claims no move.
pub fn forget() {
    target();
}

fn signal(holder: &Holder, signal: libc::c_int) -> io::Result<()> {
    let pid = libc::pid_t::try_from(holder.pid).map_err(io::Error::other)?;
    // SAFETY: `kill` reads only its two integers.
    if unsafe { libc::kill(pid, signal) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
