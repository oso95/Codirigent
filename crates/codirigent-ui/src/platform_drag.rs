//! Platform-specific window drag and resize helpers.
//!
//! On Windows, GPUI 0.2.x has two issues with custom-chrome windows:
//!
//! 1. **Drag freeze**: `WindowControlArea::Drag` causes GPUI's `WM_NCHITTEST`
//!    to return `HTCAPTION`. When Windows enters the modal drag loop via
//!    `DefWindowProc`, it re-enters the message pump while GPUI's `RefCell`
//!    borrows are still held — causing a panic / freeze.
//!
//! 2. **NC event interception**: GPUI's `handle_nc_mouse_down_msg` re-dispatches
//!    `WM_NCLBUTTONDOWN` as a regular `MouseDownEvent` through the element tree.
//!    If an element handles it (e.g., the title bar's `on_mouse_down`),
//!    `DefWindowProc` never runs — breaking both title-bar drag and top-edge
//!    resize (because the drag region overlaps the top resize zone).
//!
//! This module installs a Win32 window subclass that intercepts these messages
//! **before** GPUI's WndProc:
//!
//! - **Drag**: The `on_mouse_down` handler posts a custom `WM_APP` message.
//!   The subclass catches it and calls `DefWindowProc(WM_NCLBUTTONDOWN,
//!   HTCAPTION)` directly, starting the OS drag loop when no GPUI borrows
//!   are held.
//!
//! - **Resize**: `WM_NCLBUTTONDOWN` for resize hit-test areas (`HTTOP`,
//!   `HTLEFT`, …) is routed straight to `DefWindowProc`, bypassing GPUI's
//!   element dispatch that would otherwise eat the event.
//!
//! - **Buttons**: `WM_NCLBUTTONDOWN` for `HTMINBUTTON`/`HTMAXBUTTON`/`HTCLOSE`
//!   passes through to GPUI for normal button handling.
//!
//! Remove this module after upgrading GPUI to a version that fixes the issues.

/// Install the window subclass for drag/resize handling (Windows only).
///
/// Must be called once per window. Safe to call multiple times — only the
/// first call installs the subclass.
#[cfg(target_os = "windows")]
pub fn install_drag_subclass(hwnd: isize) {
    use std::sync::atomic::{AtomicBool, Ordering};

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    // Safety: hwnd is obtained from raw_window_handle and the window is alive.
    // SetWindowSubclass must be called from the thread that owns the window,
    // which is the UI thread where render_title_bar runs.
    unsafe {
        SetWindowSubclass(hwnd, drag_subclass_proc, SUBCLASS_ID, 0);
    }
}

/// Begin a native title-bar drag on Windows.
///
/// Posts a custom `WM_APP` message with the cursor's screen coordinates.
/// The window subclass intercepts this and calls
/// `DefWindowProc(WM_NCLBUTTONDOWN, HTCAPTION)` to start the OS drag loop.
#[cfg(target_os = "windows")]
pub fn begin_title_bar_drag(hwnd: isize) {
    // Safety: GetCursorPos and PostMessageW are safe to call from any thread,
    // and hwnd is valid during the mouse-down handler that calls this.
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        let lparam = pack_point(pt.x, pt.y);
        PostMessageW(hwnd, WM_APP_DRAG_WINDOW, 0, lparam);
    }
}

/// No-op on non-Windows platforms (drag is handled by GPUI natively).
#[cfg(not(target_os = "windows"))]
pub fn begin_title_bar_drag(_hwnd: isize) {}

/// No-op on non-Windows platforms.
#[cfg(not(target_os = "windows"))]
pub fn install_drag_subclass(_hwnd: isize) {}

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

// Win32 type aliases (avoid pulling in the full `windows` crate).
#[cfg(target_os = "windows")]
#[allow(clippy::upper_case_acronyms, non_camel_case_types)]
mod win32 {
    pub type HWND = isize;
    pub type WPARAM = usize;
    pub type LPARAM = isize;
    pub type LRESULT = isize;
    pub type BOOL = std::ffi::c_int;

    #[repr(C)]
    pub struct POINT {
        pub x: i32,
        pub y: i32,
    }

    /// Signature expected by `SetWindowSubclass` / `RemoveWindowSubclass`.
    pub type SUBCLASSPROC =
        unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM, usize, usize) -> LRESULT;
}

#[cfg(target_os = "windows")]
use win32::*;

// Message and hit-test constants.
#[cfg(target_os = "windows")]
const WM_NCLBUTTONDOWN: u32 = 0x00A1;
#[cfg(target_os = "windows")]
const WM_NCLBUTTONDBLCLK: u32 = 0x00A3;
#[cfg(target_os = "windows")]
const WM_NCDESTROY: u32 = 0x0082;
#[cfg(target_os = "windows")]
const WM_APP_DRAG_WINDOW: u32 = 0x8000; // WM_APP
#[cfg(target_os = "windows")]
const HTMINBUTTON: u32 = 8;
#[cfg(target_os = "windows")]
const HTMAXBUTTON: u32 = 9;
#[cfg(target_os = "windows")]
const HTCLOSE: u32 = 20;
#[cfg(target_os = "windows")]
const HTCAPTION: usize = 2;
#[cfg(target_os = "windows")]
const VK_LBUTTON: i32 = 0x01;
#[cfg(target_os = "windows")]
const SUBCLASS_ID: usize = 0xC0D1; // Memorable constant for our subclass.

// Imports from comctl32.dll (window subclass API).
#[cfg(target_os = "windows")]
#[link(name = "comctl32")]
extern "system" {
    fn SetWindowSubclass(
        hwnd: HWND,
        pfn_subclass: SUBCLASSPROC,
        uid_subclass: usize,
        dw_ref_data: usize,
    ) -> BOOL;
    fn RemoveWindowSubclass(hwnd: HWND, pfn_subclass: SUBCLASSPROC, uid_subclass: usize) -> BOOL;
    fn DefSubclassProc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
}

// Imports from user32.dll.
#[cfg(target_os = "windows")]
extern "system" {
    fn DefWindowProcW(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
    fn ReleaseCapture() -> BOOL;
    fn PostMessageW(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> BOOL;
    fn GetCursorPos(point: *mut POINT) -> BOOL;
    fn GetAsyncKeyState(vkey: i32) -> i16;
}

/// Pack screen coordinates as `MAKELPARAM(x, y)`.
#[cfg(target_os = "windows")]
fn pack_point(x: i32, y: i32) -> LPARAM {
    (((y & 0xFFFF) as LPARAM) << 16) | ((x & 0xFFFF) as LPARAM)
}

/// Window subclass procedure — runs **before** GPUI's WndProc.
///
/// Routes NC mouse messages so the OS handles drag/resize directly,
/// bypassing GPUI's element dispatch that would otherwise eat them.
#[cfg(target_os = "windows")]
unsafe extern "system" fn drag_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid_subclass: usize,
    _ref_data: usize,
) -> LRESULT {
    match msg {
        // Our custom drag message, posted by begin_title_bar_drag().
        // At this point no GPUI RefCell borrows are held, so the modal
        // drag loop started by DefWindowProc is safe.
        //
        // Guard: only start the drag if the left mouse button is still
        // held. On a double-click the second click fires
        // titlebar_double_click() synchronously, and by the time this
        // stale message from the first click is dequeued the button may
        // already be released — skip it to avoid a spurious drag on the
        // now-maximised window.
        WM_APP_DRAG_WINDOW => {
            if GetAsyncKeyState(VK_LBUTTON) < 0 {
                ReleaseCapture();
                DefWindowProcW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, lparam)
            } else {
                0
            }
        }

        // NC mouse-down (single or double click).
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => match wparam as u32 {
            // Window control buttons — let GPUI handle them via
            // WindowControlArea::Min/Max/Close.
            HTMINBUTTON | HTMAXBUTTON | HTCLOSE => DefSubclassProc(hwnd, msg, wparam, lparam),
            // Everything else: resize edges (HTTOP, HTLEFT, …) or a
            // stale HTCAPTION from GPUI's hit-test callback.
            // Send straight to DefWindowProc so the OS starts the
            // resize/drag loop — bypassing GPUI's element dispatch
            // which would route it to on_mouse_down and eat the event.
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        },

        // Clean up the subclass when the window is destroyed.
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, drag_subclass_proc, SUBCLASS_ID);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }

        // Everything else passes through to GPUI.
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}
