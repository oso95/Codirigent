//! Stub clipboard for non-macOS platforms.
//!
//! Provides a no-op clipboard implementation for platforms where
//! native clipboard access is not yet implemented. This allows the
//! rest of the application to compile and run on all platforms.
//!
//! ## Behavior
//!
//! - `read_content()` always returns `ClipboardContent::Empty`
//! - `write_text()` and `write_image()` are no-ops that succeed
//! - `has_image()` always returns `false`
//!
//! ## Usage
//!
//! This implementation is automatically selected on non-macOS platforms.
//! It can also be used explicitly for testing purposes.
//!
//! ```
//! use codirigent_ui::platform::StubSmartClipboard;
//! use codirigent_ui::smart_clipboard::SmartClipboardProvider;
//!
//! let clipboard = StubSmartClipboard::new();
//! assert!(!clipboard.has_image());
//! ```

use crate::smart_clipboard::SmartClipboardProvider;
use anyhow::Result;
use codirigent_core::{ClipboardContent, ImageData};

/// Stub clipboard provider for unsupported platforms.
///
/// This implementation provides no-op clipboard functionality
/// for platforms where native clipboard access is not available.
/// All read operations return empty content, and all write
/// operations succeed without effect.
///
/// # Example
///
/// ```
/// use codirigent_ui::platform::StubSmartClipboard;
/// use codirigent_ui::smart_clipboard::SmartClipboardProvider;
///
/// let clipboard = StubSmartClipboard::new();
///
/// // Always returns Empty
/// let content = clipboard.read_content().unwrap();
/// assert!(matches!(content, codirigent_core::ClipboardContent::Empty));
///
/// // Write operations succeed but do nothing
/// clipboard.write_text("test".to_string()).unwrap();
/// ```
#[derive(Debug)]
pub struct StubSmartClipboard;

impl StubSmartClipboard {
    /// Create a new stub clipboard provider.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::platform::StubSmartClipboard;
    ///
    /// let clipboard = StubSmartClipboard::new();
    /// ```
    pub fn new() -> Self {
        Self
    }
}

impl Default for StubSmartClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartClipboardProvider for StubSmartClipboard {
    /// Read current clipboard content.
    ///
    /// Always returns `ClipboardContent::Empty` for the stub implementation.
    fn read_content(&self) -> Result<ClipboardContent> {
        Ok(ClipboardContent::Empty)
    }

    /// Write text to clipboard.
    ///
    /// No-op for the stub implementation. Always succeeds.
    fn write_text(&self, _text: String) -> Result<()> {
        Ok(())
    }

    /// Write image to clipboard.
    ///
    /// No-op for the stub implementation. Always succeeds.
    fn write_image(&self, _image: &ImageData) -> Result<()> {
        Ok(())
    }

    /// Check if clipboard has image content.
    ///
    /// Always returns `false` for the stub implementation.
    fn has_image(&self) -> bool {
        false
    }

    /// Check if clipboard content has changed since last read.
    ///
    /// Always returns `false` for the stub implementation.
    fn has_changed(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_clipboard_new() {
        let clipboard = StubSmartClipboard::new();
        assert!(!clipboard.has_image());
    }

    #[test]
    fn test_stub_clipboard_default() {
        let clipboard = StubSmartClipboard::default();
        assert!(!clipboard.has_image());
    }

    #[test]
    fn test_stub_clipboard_read_content_returns_empty() {
        let clipboard = StubSmartClipboard::new();
        let content = clipboard.read_content().unwrap();
        assert!(matches!(content, ClipboardContent::Empty));
    }

    #[test]
    fn test_stub_clipboard_has_image_false() {
        let clipboard = StubSmartClipboard::new();
        assert!(!clipboard.has_image());
    }

    #[test]
    fn test_stub_clipboard_write_text_ok() {
        let clipboard = StubSmartClipboard::new();
        let result = clipboard.write_text("test content".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_stub_clipboard_write_image_ok() {
        use codirigent_core::ImageFormat;

        let clipboard = StubSmartClipboard::new();
        let image = ImageData {
            bytes: vec![0x89, 0x50, 0x4E, 0x47],
            width: 100,
            height: 100,
            format: ImageFormat::Png,
        };
        let result = clipboard.write_image(&image);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stub_clipboard_debug() {
        let clipboard = StubSmartClipboard::new();
        let debug_str = format!("{:?}", clipboard);
        assert!(debug_str.contains("StubSmartClipboard"));
    }

    #[test]
    fn test_stub_clipboard_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StubSmartClipboard>();
    }

    #[test]
    fn test_stub_clipboard_multiple_writes() {
        let clipboard = StubSmartClipboard::new();

        // Multiple writes should all succeed
        clipboard.write_text("first".to_string()).unwrap();
        clipboard.write_text("second".to_string()).unwrap();
        clipboard.write_text("third".to_string()).unwrap();

        // Reading should still return Empty
        let content = clipboard.read_content().unwrap();
        assert!(matches!(content, ClipboardContent::Empty));
    }
}
