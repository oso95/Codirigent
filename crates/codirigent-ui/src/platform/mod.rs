//! Platform-specific implementations.
//!
//! This module contains platform-specific code for clipboard access
//! and other OS-level functionality. The appropriate implementation
//! is selected at compile time based on the target platform.
//!
//! ## Supported Platforms
//!
//! - **macOS**: Full clipboard support with NSPasteboard (MVP: stub)
//! - **Windows**: Full clipboard support with Win32 APIs via clipboard-win
//! - **Linux**: Full clipboard support with arboard (X11/Wayland)
//! - **Other platforms**: Stub implementation returning empty content
//!
//! ## Usage
//!
//! ```no_run
//! use codirigent_ui::platform;
//!
//! // On macOS, this is MacOSSmartClipboard
//! // On Windows, this is WindowsSmartClipboard
//! // On Linux, this is LinuxSmartClipboard
//! // On other platforms, this is StubSmartClipboard
//! #[cfg(target_os = "macos")]
//! let clipboard = platform::MacOSSmartClipboard::new();
//!
//! #[cfg(target_os = "windows")]
//! let clipboard = platform::WindowsSmartClipboard::new();
//!
//! #[cfg(target_os = "linux")]
//! let clipboard = platform::LinuxSmartClipboard::new().unwrap();
//!
//! #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
//! let clipboard = platform::StubSmartClipboard::new();
//! ```

#[cfg(target_os = "macos")]
mod clipboard_macos;

#[cfg(target_os = "macos")]
pub use clipboard_macos::MacOSSmartClipboard;

#[cfg(target_os = "windows")]
mod clipboard_windows;

#[cfg(target_os = "windows")]
pub use clipboard_windows::WindowsSmartClipboard;

// Linux clipboard implementation
#[cfg(target_os = "linux")]
mod clipboard_linux;

#[cfg(target_os = "linux")]
pub use clipboard_linux::LinuxSmartClipboard;

// Stub for platforms without native clipboard support
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod clipboard_stub;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub use clipboard_stub::StubSmartClipboard;

// Always include the stub module for testing purposes on all platforms
// This allows tests to use StubSmartClipboard regardless of platform
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod clipboard_stub;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub use clipboard_stub::StubSmartClipboard;

/// Create a platform-appropriate smart clipboard provider.
///
/// Returns the correct implementation for the current platform:
/// - macOS: `MacOSSmartClipboard`
/// - Windows: `WindowsSmartClipboard`
/// - Linux: `LinuxSmartClipboard` (fallback to stub on error)
/// - Other: `StubSmartClipboard`
///
/// # Example
///
/// ```no_run
/// use codirigent_ui::platform::create_clipboard;
/// use codirigent_ui::smart_clipboard::SmartClipboardProvider;
///
/// let clipboard = create_clipboard();
/// let content = clipboard.read_content().unwrap();
/// ```
#[cfg(target_os = "macos")]
pub fn create_clipboard() -> Box<dyn crate::smart_clipboard::SmartClipboardProvider> {
    Box::new(MacOSSmartClipboard::new())
}

/// Create a platform-appropriate smart clipboard provider.
///
/// Returns the correct implementation for the current platform:
/// - macOS: `MacOSSmartClipboard`
/// - Windows: `WindowsSmartClipboard`
/// - Linux: `LinuxSmartClipboard` (fallback to stub on error)
/// - Other: `StubSmartClipboard`
///
/// # Example
///
/// ```ignore
/// use codirigent_ui::platform::create_clipboard;
/// use codirigent_ui::smart_clipboard::SmartClipboardProvider;
///
/// let clipboard = create_clipboard();
/// let content = clipboard.read_content().unwrap();
/// ```
#[cfg(target_os = "windows")]
pub fn create_clipboard() -> Box<dyn crate::smart_clipboard::SmartClipboardProvider> {
    Box::new(WindowsSmartClipboard::new())
}

/// Create a platform-appropriate smart clipboard provider.
///
/// Returns the correct implementation for the current platform:
/// - macOS: `MacOSSmartClipboard`
/// - Windows: `WindowsSmartClipboard`
/// - Linux: `LinuxSmartClipboard` (fallback to stub on error)
/// - Other: `StubSmartClipboard`
///
/// On Linux, this attempts to create a LinuxSmartClipboard. If that fails
/// (e.g., no X11/Wayland display available), it falls back to StubSmartClipboard.
///
/// # Example
///
/// ```no_run
/// use codirigent_ui::platform::create_clipboard;
/// use codirigent_ui::smart_clipboard::SmartClipboardProvider;
///
/// let clipboard = create_clipboard();
/// let content = clipboard.read_content().unwrap();
/// ```
#[cfg(target_os = "linux")]
pub fn create_clipboard() -> Box<dyn crate::smart_clipboard::SmartClipboardProvider> {
    match LinuxSmartClipboard::new() {
        Ok(clipboard) => Box::new(clipboard),
        Err(_) => Box::new(StubSmartClipboard::new()),
    }
}

/// Create a platform-appropriate smart clipboard provider.
///
/// Returns the correct implementation for the current platform:
/// - macOS: `MacOSSmartClipboard`
/// - Windows: `WindowsSmartClipboard`
/// - Linux: `LinuxSmartClipboard`
/// - Other: `StubSmartClipboard`
///
/// # Example
///
/// ```
/// use codirigent_ui::platform::create_clipboard;
/// use codirigent_ui::smart_clipboard::SmartClipboardProvider;
///
/// let clipboard = create_clipboard();
/// let content = clipboard.read_content().unwrap();
/// ```
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn create_clipboard() -> Box<dyn crate::smart_clipboard::SmartClipboardProvider> {
    Box::new(StubSmartClipboard::new())
}

/// Try to create a Linux smart clipboard provider.
///
/// This is a convenience function for Linux that returns a Result,
/// allowing callers to handle initialization failures gracefully.
///
/// # Example
///
/// ```no_run
/// use codirigent_ui::platform::try_create_linux_clipboard;
/// use codirigent_ui::smart_clipboard::SmartClipboardProvider;
///
/// match try_create_linux_clipboard() {
///     Ok(clipboard) => {
///         let content = clipboard.read_content().unwrap();
///         // Use clipboard...
///     }
///     Err(e) => {
///         eprintln!("Clipboard not available: {}", e);
///     }
/// }
/// ```
#[cfg(target_os = "linux")]
pub fn try_create_linux_clipboard() -> anyhow::Result<LinuxSmartClipboard> {
    LinuxSmartClipboard::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smart_clipboard::SmartClipboardProvider;
    #[allow(unused_imports)]
    use codirigent_core::ClipboardContent;
    use serial_test::serial;

    #[test]
    #[serial(clipboard)]
    #[cfg(target_os = "macos")]
    fn test_create_clipboard_macos() {
        let clipboard = create_clipboard();
        // On macOS, clipboard may have content - we just verify it doesn't error
        let content = clipboard.read_content();
        assert!(content.is_ok());
    }

    #[test]
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    fn test_create_clipboard_stub() {
        let clipboard = create_clipboard();
        let content = clipboard.read_content().unwrap();
        // Stub implementation should return Empty
        assert!(matches!(content, ClipboardContent::Empty));
    }

    #[test]
    #[serial(clipboard)]
    #[cfg(target_os = "windows")]
    fn test_create_clipboard_windows() {
        let clipboard = create_clipboard();
        // On Windows, clipboard may or may not have content
        let content = clipboard.read_content();
        assert!(content.is_ok());
    }

    #[test]
    #[serial(clipboard)]
    #[cfg(target_os = "linux")]
    fn test_create_clipboard_linux() {
        let clipboard = create_clipboard();
        // On Linux, clipboard may or may not have content depending on display availability
        let content = clipboard.read_content();
        assert!(content.is_ok());
    }

    #[test]
    fn test_stub_clipboard_available() {
        // StubSmartClipboard should be available on all platforms for testing
        let stub = StubSmartClipboard::new();
        assert!(!stub.has_image());
    }

    #[test]
    #[serial(clipboard)]
    #[cfg(target_os = "windows")]
    fn test_windows_clipboard_available() {
        // WindowsSmartClipboard should be available on Windows
        let clipboard = WindowsSmartClipboard::new();
        // Should not panic
        let _ = clipboard.has_image();
    }

    #[test]
    #[serial(clipboard)]
    #[cfg(target_os = "windows")]
    fn test_windows_clipboard_has_changed() {
        let clipboard = WindowsSmartClipboard::new();
        // First call captures initial state
        let _ = clipboard.has_changed();
        // Second call without external changes should return false
        let changed = clipboard.has_changed();
        assert!(!changed);
    }

    #[test]
    #[serial(clipboard)]
    #[cfg(target_os = "linux")]
    fn test_linux_clipboard_try_create() {
        let result = try_create_linux_clipboard();
        // May succeed or fail depending on display availability
        match result {
            Ok(clipboard) => {
                // Verify we can call methods on the clipboard
                let _ = clipboard.has_image();
            }
            Err(e) => {
                // Expected in CI environments without a display
                eprintln!("Expected in CI: {}", e);
            }
        }
    }

    #[test]
    #[serial(clipboard)]
    #[cfg(target_os = "linux")]
    fn test_linux_create_clipboard_returns_box() {
        let clipboard = create_clipboard();
        // Verify we can use the trait object
        let _ = clipboard.has_image();
    }
}
