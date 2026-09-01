//! Cross-platform shutdown guard.
//!
//! Prevents the operating system from terminating the application during
//! logout or shutdown when sessions are actively running.
//!
//! ## Platform behavior
//!
//! - **macOS**: Adds `applicationShouldTerminate:` to GPUI's delegate,
//!   returning `NSTerminateCancel` when sessions are active. macOS shows
//!   "Your Mac hasn't logged out because Codirigent failed to quit."
//!
//! - **Windows**: Subclasses the GPUI window to intercept `WM_QUERYENDSESSION`,
//!   returning `FALSE` and setting a `ShutdownBlockReason` when sessions are active.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the application should block system shutdown/logout.
///
/// When `true`, the OS will be prevented from terminating the app during
/// shutdown/logout, showing a system dialog to the user.
static SHUTDOWN_BLOCKED: AtomicBool = AtomicBool::new(false);

/// Whether the current quit was initiated by the user (Cmd+Q / Ctrl+Q).
///
/// On macOS, `[NSApp terminate:]` triggers `applicationShouldTerminate:` for
/// both user-initiated and system-initiated quits. This flag distinguishes
/// the two so that Cmd+Q always quits immediately.
///
/// Not needed on Windows — `WM_QUERYENDSESSION` is only sent during
/// system shutdown, never during user-initiated quit.
#[cfg(target_os = "macos")]
pub(crate) static USER_QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Update whether the app should block system shutdown.
///
/// Call this whenever the session count changes:
/// - `true` when there are active sessions
/// - `false` when all sessions are closed
pub fn set_shutdown_blocked(blocked: bool) {
    SHUTDOWN_BLOCKED.store(blocked, Ordering::SeqCst);
}

/// Check if shutdown is currently blocked.
pub fn is_shutdown_blocked() -> bool {
    SHUTDOWN_BLOCKED.load(Ordering::SeqCst)
}

/// Install the platform-specific shutdown guard.
///
/// On macOS, this adds `applicationShouldTerminate:` to GPUI's delegate class.
/// Must be called inside `Application::new().run()` after GPUI has initialized.
#[cfg(target_os = "macos")]
pub fn install() {
    super::shutdown_guard_macos::install_shutdown_guard();
}

/// No-op on non-macOS platforms. Windows uses `install_for_window` instead.
#[cfg(not(target_os = "macos"))]
pub fn install() {}

/// Install the Windows shutdown guard by subclassing the given window.
///
/// Must be called after the GPUI window has been created, from within the
/// `open_window` callback where the raw window handle is available.
#[cfg(target_os = "windows")]
pub fn install_for_window(hwnd: isize) {
    super::shutdown_guard_windows::install_shutdown_guard(hwnd);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_blocked_default_false() {
        // Reset to known state (other tests may have changed it)
        set_shutdown_blocked(false);
        assert!(!is_shutdown_blocked());
    }

    #[test]
    fn test_set_shutdown_blocked() {
        set_shutdown_blocked(true);
        assert!(is_shutdown_blocked());
        set_shutdown_blocked(false);
        assert!(!is_shutdown_blocked());
    }
}
