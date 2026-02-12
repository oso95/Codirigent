//! Platform-specific window drag helpers.
//!
//! On Windows, GPUI 0.2.x has a timing issue where `WindowControlArea::Drag`
//! doesn't reliably initiate window moves (stale `mouse_hit_test` in
//! WM_NCHITTEST). This module provides a direct Win32 workaround that sends
//! `WM_NCLBUTTONDOWN` with `HTCAPTION` to the window, telling Windows to
//! start a native title-bar drag.
//!
//! Remove this module after upgrading GPUI to a version that fixes the issue.

/// Begin a native title-bar drag on Windows.
///
/// Posts `ReleaseCapture` + `WM_NCLBUTTONDOWN(HTCAPTION)` so the OS
/// takes over the drag loop. Uses `PostMessageW` (async) instead of
/// `SendMessageW` (sync) to avoid reentrancy — `SendMessageW` starts a
/// modal drag loop that pumps messages while GPUI's `RefCell` is still
/// borrowed by the event callback, causing a panic.
#[cfg(target_os = "windows")]
pub fn begin_title_bar_drag(hwnd: isize) {
    use std::ffi::c_int;

    #[allow(clippy::upper_case_acronyms)]
    type HWND = isize;
    #[allow(clippy::upper_case_acronyms)]
    type WPARAM = usize;
    #[allow(clippy::upper_case_acronyms)]
    type LPARAM = isize;
    #[allow(clippy::upper_case_acronyms)]
    type BOOL = c_int;

    const WM_NCLBUTTONDOWN: u32 = 0x00A1;
    const HTCAPTION: WPARAM = 2;

    extern "system" {
        fn ReleaseCapture() -> BOOL;
        fn PostMessageW(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> BOOL;
    }

    // Safety: hwnd is obtained from raw_window_handle and the window is alive
    // during the mouse-down handler that calls this function.
    unsafe {
        ReleaseCapture();
        PostMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, 0);
    }
}

/// No-op on non-Windows platforms (drag is handled by GPUI natively).
#[cfg(not(target_os = "windows"))]
pub fn begin_title_bar_drag(_hwnd: isize) {}
