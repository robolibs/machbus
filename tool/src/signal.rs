//! Cooperative Ctrl-C handling.
//!
//! Installs a SIGINT/SIGTERM handler that flips an atomic flag; long-lived
//! loops (`dump`, `gen`) poll [`cancel_requested`] between iterations so
//! they can stop cleanly and print a summary instead of being killed
//! mid-write.

use std::sync::atomic::{AtomicBool, Ordering};

static CANCEL: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_sig: libc::c_int) {
    CANCEL.store(true, Ordering::Release);
}

/// Install the handler. Safe to call once at program start. On non-Unix
/// targets this is a no-op.
pub fn install_cancel_handler() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
    }
}

/// Whether a cancellation signal has been observed.
#[must_use]
pub fn cancel_requested() -> bool {
    CANCEL.load(Ordering::Acquire)
}
