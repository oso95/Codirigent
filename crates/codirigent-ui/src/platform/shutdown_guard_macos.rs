//! macOS shutdown guard implementation.
//!
//! Adds `applicationShouldTerminate:` to GPUI's `GPUIApplicationDelegate`
//! class via the ObjC runtime. When sessions are active and the system
//! tries to shut down or log out, this returns `NSTerminateCancel`,
//! causing macOS to show the "Your Mac hasn't logged out because
//! Codirigent failed to quit" dialog.
//!
//! User-initiated quits (Cmd+Q) are always allowed through, distinguished
//! by the `USER_QUIT_REQUESTED` flag set in the Quit action handler.

use super::shutdown_guard::{is_shutdown_blocked, USER_QUIT_REQUESTED};
use std::ffi::c_char;
use std::sync::atomic::Ordering;
use tracing::{info, warn};

/// `NSTerminateCancel` — The app should not be terminated.
const NS_TERMINATE_CANCEL: usize = 0;
/// `NSTerminateNow` — It is OK to proceed with termination.
const NS_TERMINATE_NOW: usize = 1;

// ObjC runtime functions for adding a method to an existing class.
extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut std::ffi::c_void;
    fn sel_registerName(name: *const c_char) -> *const std::ffi::c_void;
    fn class_addMethod(
        cls: *mut std::ffi::c_void,
        name: *const std::ffi::c_void,
        imp: extern "C" fn(
            *mut std::ffi::c_void,
            *const std::ffi::c_void,
            *mut std::ffi::c_void,
        ) -> usize,
        types: *const c_char,
    ) -> i8;
}

/// Install the macOS shutdown guard.
///
/// Adds `applicationShouldTerminate:` to GPUI's `GPUIApplicationDelegate`
/// class. This must be called after GPUI has initialized (inside
/// `Application::new().run()`), but before the app enters the event loop.
///
/// # Safety
///
/// Uses the ObjC runtime to modify the `GPUIApplicationDelegate` class.
/// The class must exist (GPUI creates it via `#[ctor]` before `main()`).
pub fn install_shutdown_guard() {
    unsafe {
        let cls = objc_getClass(b"GPUIApplicationDelegate\0".as_ptr().cast::<c_char>());
        if cls.is_null() {
            warn!("Could not find GPUIApplicationDelegate class; shutdown guard not installed");
            return;
        }

        let sel = sel_registerName(b"applicationShouldTerminate:\0".as_ptr().cast::<c_char>());

        // Type encoding: return NSUInteger (Q), self (@), _cmd (:), sender (@)
        let types = b"Q@:@\0".as_ptr().cast::<c_char>();

        let added = class_addMethod(cls, sel, application_should_terminate, types);

        if added != 0 {
            info!("Shutdown guard installed on GPUIApplicationDelegate");
        } else {
            warn!(
                "applicationShouldTerminate: already exists on GPUIApplicationDelegate; \
                 shutdown guard not installed"
            );
        }
    }
}

/// ObjC method implementation for `applicationShouldTerminate:`.
///
/// Called by macOS when the system attempts to terminate the app (shutdown,
/// logout, or `[NSApp terminate:]`).
extern "C" fn application_should_terminate(
    _this: *mut std::ffi::c_void,
    _sel: *const std::ffi::c_void,
    _sender: *mut std::ffi::c_void,
) -> usize {
    // If the user explicitly requested quit (Cmd+Q), allow unconditionally.
    // Reset the flag so subsequent system-initiated attempts are not affected.
    if USER_QUIT_REQUESTED.swap(false, Ordering::SeqCst) {
        info!("User-initiated quit: allowing termination");
        return NS_TERMINATE_NOW;
    }

    // System-initiated (shutdown/logout): block if sessions are active.
    if is_shutdown_blocked() {
        info!("Blocking system shutdown: active sessions exist");
        NS_TERMINATE_CANCEL
    } else {
        info!("No active sessions: allowing system termination");
        NS_TERMINATE_NOW
    }
}
