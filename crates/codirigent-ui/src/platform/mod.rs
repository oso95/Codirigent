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
//! - **Other platforms**: Stub implementation returning empty content
//!
//! ## Usage
//!
//! ```no_run
//! use codirigent_ui::platform;
//!
//! // On macOS, this is MacOSSmartClipboard
//! // On Windows, this is WindowsSmartClipboard
//! // On other platforms, this is StubSmartClipboard
//! #[cfg(target_os = "macos")]
//! let clipboard = platform::MacOSSmartClipboard::new();
//!
//! #[cfg(target_os = "windows")]
//! let clipboard = platform::WindowsSmartClipboard::new();
//!
//! #[cfg(not(any(target_os = "macos", target_os = "windows")))]
//! let clipboard = platform::StubSmartClipboard::new();
//! ```

// Shutdown guard — prevents system shutdown/logout while sessions are active
pub mod shutdown_guard;

#[cfg(target_os = "macos")]
mod shutdown_guard_macos;

#[cfg(target_os = "windows")]
mod shutdown_guard_windows;

#[cfg(target_os = "macos")]
mod clipboard_macos;

#[cfg(target_os = "macos")]
pub use clipboard_macos::MacOSSmartClipboard;

#[cfg(target_os = "windows")]
mod clipboard_windows;

#[cfg(target_os = "windows")]
pub use clipboard_windows::WindowsSmartClipboard;

// Stub for platforms without native clipboard support
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod clipboard_stub;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use clipboard_stub::StubSmartClipboard;

// Always include the stub module for testing purposes on all platforms
// This allows tests to use StubSmartClipboard regardless of platform
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod clipboard_stub;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub use clipboard_stub::StubSmartClipboard;

/// Create a platform-appropriate smart clipboard provider.
///
/// Returns the correct implementation for the current platform:
/// - macOS: `MacOSSmartClipboard`
/// - Windows: `WindowsSmartClipboard`
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
pub fn create_clipboard() -> std::sync::Arc<dyn crate::smart_clipboard::SmartClipboardProvider> {
    std::sync::Arc::new(MacOSSmartClipboard::new())
}

/// See macOS variant for full documentation.
#[cfg(target_os = "windows")]
pub fn create_clipboard() -> std::sync::Arc<dyn crate::smart_clipboard::SmartClipboardProvider> {
    std::sync::Arc::new(WindowsSmartClipboard::new())
}

/// See macOS variant for full documentation.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn create_clipboard() -> std::sync::Arc<dyn crate::smart_clipboard::SmartClipboardProvider> {
    std::sync::Arc::new(StubSmartClipboard::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smart_clipboard::SmartClipboardProvider;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
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
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
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
        // Smoke test only: CI environments may have transient clipboard access
        // restrictions. Validate core API calls don't panic.
        let _ = clipboard.has_image();
        let _ = clipboard.has_changed();
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
}
