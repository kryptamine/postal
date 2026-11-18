//! Allocations per frame, counted only with the test-only `alloc-count`
//! feature; without it the system allocator stays and nothing is logged.

#[cfg(all(feature = "alloc-count", not(debug_assertions)))]
compile_error!("alloc-count is test-only: release builds keep the system allocator");

#[cfg(feature = "alloc-count")]
mod counting {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };

    // Per thread: the log writer and the terminal reader run on their own
    // threads, and only the UI thread draws frames.
    thread_local! {
        static COUNT: Cell<u64> = const { Cell::new(0) };
    }

    struct Counting;

    fn bump() {
        let _ = COUNT.try_with(|count| count.set(count.get() + 1));
    }

    // SAFETY: every call is forwarded unchanged to the system allocator.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `GlobalAlloc`'s contract, passed on as is.
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `GlobalAlloc`'s contract, passed on as is.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            bump();
            // SAFETY: the caller upholds `GlobalAlloc`'s contract, passed on as is.
            unsafe { System.realloc(ptr, layout, new_size) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: the caller upholds `GlobalAlloc`'s contract, passed on as is.
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static GLOBAL: Counting = Counting;

    /// Allocations made on the calling thread so far.
    pub fn count() -> u64 {
        COUNT.with(Cell::get)
    }
}

#[cfg(feature = "alloc-count")]
pub use counting::count;

/// Logs the allocations made since `start`, a reading of `count`. The event
/// itself allocates, so it is sent after the reading.
#[cfg(feature = "alloc-count")]
pub fn report(start: u64) {
    let allocations = count() - start;
    tracing::info!(allocations, "frame drawn");
}

#[cfg(not(feature = "alloc-count"))]
pub const fn count() -> u64 {
    0
}

#[cfg(not(feature = "alloc-count"))]
pub const fn report(_start: u64) {}
