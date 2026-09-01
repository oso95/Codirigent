//! Windows shutdown guard implementation.
//!
//! Subclasses the GPUI window to intercept `WM_QUERYENDSESSION`.
//! When sessions are active and the system tries to shut down or
//! log out, this returns `FALSE` and sets a `ShutdownBlockReason`,
//! causing Windows to show a dialog telling the user that
//! Codirigent is preventing shutdown.

use super::shutdown_guard::is_shutdown_blocked;
use std::sync::atomic::{AtomicIsize, Ordering};
use tracing::{info, warn};

const GWLP_WNDPROC: i32 = -4;
const WM_QUERYENDSESSION: u32 = 0x0011;

/// The original window procedure, saved when subclassing.
static ORIGINAL_WNDPROC: AtomicIsize = AtomicIsize::new(0);

// Win32 functions from user32.dll (linked automatically on Windows).
extern "system" {
    fn SetWindowLongPtrW(hwnd: isize, index: i32, new_long: isize) -> isize;
    fn CallWindowProcW(prev: isize, hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn ShutdownBlockReasonCreate(hwnd: isize, reason: *const u16) -> i32;
    fn ShutdownBlockReasonDestroy(hwnd: isize) -> i32;
}

/// Install the Windows shutdown guard by subclassing the given window.
///
/// Replaces the window procedure with one that intercepts `WM_QUERYENDSESSION`
/// and blocks shutdown when sessions are active.
///
/// Must be called after the GPUI window has been created, from the main thread.
pub fn install_shutdown_guard(hwnd: isize) {
    if hwnd == 0 {
        warn!("Invalid HWND (null); shutdown guard not installed");
        return;
    }

    let prev = unsafe {
        SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            shutdown_guard_wndproc as *const () as isize,
        )
    };
    if prev == 0 {
        warn!("Failed to subclass window for shutdown guard");
        return;
    }

    ORIGINAL_WNDPROC.store(prev, Ordering::SeqCst);
    info!("Shutdown guard installed on window");
}

/// Subclassed window procedure that intercepts `WM_QUERYENDSESSION`.
///
/// When the system tries to shut down or log out:
/// - If sessions are active: sets a `ShutdownBlockReason` and returns `FALSE` (0)
///   to block shutdown. Windows shows the reason to the user.
/// - If no sessions: cleans up any previous block reason and forwards to
///   the original GPUI window procedure.
///
/// All other messages are forwarded to the original window procedure unchanged.
unsafe extern "system" fn shutdown_guard_wndproc(
    hwnd: isize,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if msg == WM_QUERYENDSESSION {
        if is_shutdown_blocked() {
            info!("Blocking system shutdown: active sessions exist");

            // Encode reason as null-terminated UTF-16 for the Windows API.
            let reason: Vec<u16> = "Codirigent has active AI sessions"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            unsafe { ShutdownBlockReasonCreate(hwnd, reason.as_ptr()) };

            return 0; // FALSE — block shutdown
        }

        // Not blocking: clean up any stale block reason from a previous attempt.
        unsafe { ShutdownBlockReasonDestroy(hwnd) };
    }

    let prev = ORIGINAL_WNDPROC.load(Ordering::SeqCst);
    unsafe { CallWindowProcW(prev, hwnd, msg, wparam, lparam) }
}
