//! The TUI's lock and what a takeover from another terminal waits on,
//! shared by the event loop and the thread that hears SIGTERM.

use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use tokio::sync::Notify;

use crate::mail::sync_lock::{Busy, SyncLock};

static HELD: Mutex<Option<SyncLock>> = Mutex::new(None);
static OUTSIDE: AtomicBool = AtomicBool::new(false);
static SENDING: AtomicUsize = AtomicUsize::new(0);
static LEAVING: AtomicBool = AtomicBool::new(false);
static RELEASED: AtomicBool = AtomicBool::new(false);
static SERVING: AtomicBool = AtomicBool::new(false);
static WAKE: Notify = Notify::const_new();

/// Keeps `lock` until the process ends or `release` lets it go.
pub fn hold(lock: SyncLock) {
    *held() = Some(lock);
}

/// Lets the lock go for good, so another terminal's TUI takes it.
pub fn release() {
    RELEASED.store(true, Ordering::SeqCst);
    held().take();
}

/// Whether the lock was let go while the TUI still ran.
pub fn released() -> bool {
    RELEASED.load(Ordering::SeqCst)
}

/// Another terminal asked for the session: no new Send starts.
pub fn leave() {
    LEAVING.store(true, Ordering::SeqCst);
    WAKE.notify_one();
}

pub fn leaving() -> bool {
    LEAVING.load(Ordering::SeqCst)
}

pub fn sending() -> bool {
    SENDING.load(Ordering::SeqCst) > 0
}

pub fn outside() -> bool {
    OUTSIDE.load(Ordering::SeqCst)
}

/// Wakes after `leave`, or once a Send ends.
pub async fn woken() {
    WAKE.notified().await;
}

pub fn serving() -> bool {
    SERVING.load(Ordering::SeqCst)
}

/// While alive, the event loop runs and answers `leave`.
pub struct Serving(());

impl Serving {
    pub fn start() -> Self {
        SERVING.store(true, Ordering::SeqCst);
        Self(())
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        SERVING.store(false, Ordering::SeqCst);
    }
}

/// While alive, a program has the terminal.
pub struct Outside(());

impl Outside {
    pub fn enter() -> Self {
        OUTSIDE.store(true, Ordering::SeqCst);
        note();
        Self(())
    }
}

impl Drop for Outside {
    fn drop(&mut self) {
        OUTSIDE.store(false, Ordering::SeqCst);
        note();
    }
}

/// While alive, SMTP has a Message not yet accepted and filed.
pub struct Sending(());

impl Sending {
    pub fn start() -> Self {
        SENDING.fetch_add(1, Ordering::SeqCst);
        note();
        Self(())
    }
}

impl Drop for Sending {
    fn drop(&mut self) {
        SENDING.fetch_sub(1, Ordering::SeqCst);
        note();
        WAKE.notify_one();
    }
}

/// Tells the lock file what the TUI is doing, for a takeover to show.
fn note() {
    let busy = Busy {
        outside: outside(),
        sending: sending(),
    };

    if let Some(lock) = held().as_ref()
        && let Err(error) = lock.note(busy)
    {
        tracing::warn!("writing the lock's holder failed: {error}");
    }
}

/// A thread that panicked holding it left the lock as it was.
fn held() -> MutexGuard<'static, Option<SyncLock>> {
    HELD.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
