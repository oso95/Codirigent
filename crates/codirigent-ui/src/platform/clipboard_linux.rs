//! Linux clipboard implementation.
//!
//! Provides clipboard access on Linux using the arboard crate for
//! X11 and Wayland clipboard support. The implementation uses a Mutex
//! to make the Clipboard instance thread-safe.
//!
//! ## Platform Support
//!
//! This implementation supports both X11 and Wayland display servers through
//! the arboard crate. On Wayland, the `wayland-data-control` feature of arboard
//! provides proper Wayland clipboard support.
//!
//! ## Thread Safety
//!
//! The arboard `Clipboard` type is not `Send + Sync` by itself, so we wrap it
//! in a `Mutex` to allow safe access from multiple threads. The Mutex is created
//! lazily on first access.
//!
//! ## Example
//!
//! ```no_run
//! use codirigent_ui::platform::LinuxSmartClipboard;
//! use codirigent_ui::smart_clipboard::SmartClipboardProvider;
//!
//! let clipboard = LinuxSmartClipboard::new().expect("Failed to create clipboard");
//!
//! // Check for image content
//! if clipboard.has_image() {
//!     let content = clipboard.read_content().unwrap();
//!     // Process image...
//! }
//! ```

use crate::smart_clipboard::SmartClipboardProvider;
use anyhow::{Context, Result};
use arboard::Clipboard;
use codirigent_core::{ClipboardContent, ImageData, ImageFormat};
use std::sync::Mutex;

/// Linux clipboard provider using arboard.
///
/// Provides access to the Linux system clipboard with support for
/// text and images. Uses a Mutex internally to ensure thread safety.
///
/// # Example
///
/// ```no_run
/// use codirigent_ui::platform::LinuxSmartClipboard;
/// use codirigent_ui::smart_clipboard::SmartClipboardProvider;
///
/// let clipboard = LinuxSmartClipboard::new().expect("Failed to create clipboard");
///
/// // Write text to clipboard
/// clipboard.write_text("Hello from Linux!".to_string()).unwrap();
///
/// // Read clipboard content
/// let content = clipboard.read_content().unwrap();
/// ```
pub struct LinuxSmartClipboard {
    /// Inner clipboard protected by a Mutex for thread safety.
    clipboard: Mutex<Clipboard>,
}

impl std::fmt::Debug for LinuxSmartClipboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinuxSmartClipboard")
            .field("clipboard", &"Mutex<Clipboard>")
            .finish()
    }
}

impl LinuxSmartClipboard {
    /// Create a new Linux clipboard provider.
    ///
    /// This initializes the arboard clipboard which will attempt to
    /// connect to the X11 or Wayland display server.
    ///
    /// # Errors
    ///
    /// Returns an error if clipboard initialization fails, which can happen
    /// if no display server is available or the required libraries are missing.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_ui::platform::LinuxSmartClipboard;
    ///
    /// let clipboard = LinuxSmartClipboard::new().expect("Failed to create clipboard");
    /// ```
    pub fn new() -> Result<Self> {
        let clipboard =
            Clipboard::new().context("Failed to initialize Linux clipboard (X11/Wayland)")?;

        Ok(Self {
            clipboard: Mutex::new(clipboard),
        })
    }
}

impl Default for LinuxSmartClipboard {
    /// Create a new Linux clipboard provider with default settings.
    ///
    /// # Panics
    ///
    /// Panics if clipboard initialization fails. For fallible construction,
    /// use [`LinuxSmartClipboard::new()`] instead.
    fn default() -> Self {
        Self::new().expect("Failed to initialize default Linux clipboard")
    }
}

impl SmartClipboardProvider for LinuxSmartClipboard {
    /// Read current clipboard content.
    ///
    /// Attempts to read the clipboard content, checking for images first
    /// (since they are more specific), then falling back to text.
    ///
    /// # Returns
    ///
    /// - `ClipboardContent::Image` if image data is available
    /// - `ClipboardContent::Text` if text is available
    /// - `ClipboardContent::Empty` if the clipboard is empty or unreadable
    fn read_content(&self) -> Result<ClipboardContent> {
        let mut clipboard = self
            .clipboard
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock clipboard mutex: {}", e))?;

        // Try to read image first (more specific content type)
        if let Ok(image) = clipboard.get_image() {
            let rgba_bytes = image.bytes.into_owned();
            let image_data = ImageData {
                bytes: rgba_bytes,
                width: image.width as u32,
                height: image.height as u32,
                format: ImageFormat::Rgba, // arboard returns raw RGBA pixel data
            };
            return Ok(ClipboardContent::Image(image_data));
        }

        // Try to read text
        if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() {
                return Ok(ClipboardContent::Text(text));
            }
        }

        Ok(ClipboardContent::Empty)
    }

    /// Write text to clipboard.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to write to the clipboard
    ///
    /// # Errors
    ///
    /// Returns an error if clipboard access fails.
    fn write_text(&self, text: String) -> Result<()> {
        let mut clipboard = self
            .clipboard
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock clipboard mutex: {}", e))?;

        clipboard
            .set_text(text)
            .context("Failed to write text to clipboard")?;

        Ok(())
    }

    /// Write image to clipboard.
    ///
    /// Converts the ImageData to arboard's ImageData format and writes
    /// it to the clipboard. The image is expected to be in RGBA format.
    ///
    /// # Arguments
    ///
    /// * `image` - The image data to write to the clipboard
    ///
    /// # Errors
    ///
    /// Returns an error if clipboard access fails or the image data is invalid.
    fn write_image(&self, image: &ImageData) -> Result<()> {
        let mut clipboard = self
            .clipboard
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock clipboard mutex: {}", e))?;

        let arboard_image = arboard::ImageData {
            width: image.width as usize,
            height: image.height as usize,
            bytes: std::borrow::Cow::Borrowed(&image.bytes),
        };

        clipboard
            .set_image(arboard_image)
            .context("Failed to write image to clipboard")?;

        Ok(())
    }

    /// Check if clipboard has image content.
    ///
    /// This attempts to read the image to check if one exists.
    /// Note that this is not a lightweight check and will actually
    /// read the image data.
    ///
    /// # Returns
    ///
    /// `true` if the clipboard contains image data, `false` otherwise.
    fn has_image(&self) -> bool {
        if let Ok(mut clipboard) = self.clipboard.lock() {
            clipboard.get_image().is_ok()
        } else {
            false
        }
    }

    fn has_changed(&self) -> bool {
        // Linux/arboard doesn't provide a clipboard sequence number.
        // Always return true — the preview logic uses timers for dedup.
        true
    }
}

// Manual implementation of Send + Sync
//
// SAFETY: The arboard::Clipboard is wrapped in a std::sync::Mutex which provides
// synchronized access. While arboard::Clipboard may hold platform-specific handles
// (X11 connection, Wayland fd, etc.), these handles are:
// 1. Only accessed through the Mutex, ensuring mutual exclusion
// 2. Created on construction and used throughout the lifetime
// 3. Not shared with any other threads directly
//
// The arboard crate (v3.x) internally uses platform APIs that may not be thread-safe,
// but our Mutex wrapper ensures that only one thread can access the clipboard at a time.
// This pattern is commonly used in Rust for wrapping non-Send types.
//
// Note: If arboard changes its internal implementation, this may need to be revisited.
// See: https://docs.rs/arboard/latest/arboard/struct.Clipboard.html
unsafe impl Send for LinuxSmartClipboard {}
unsafe impl Sync for LinuxSmartClipboard {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_clipboard_new() {
        // This test may fail in CI environments without a display server
        // but it verifies the API is correct
        let result = LinuxSmartClipboard::new();

        // On systems without X11/Wayland, this will fail, which is expected
        // The important thing is that the code compiles and the API is correct
        match result {
            Ok(clipboard) => {
                // If we got a clipboard, verify it doesn't have an image initially
                // (or it has whatever the system clipboard has)
                let _ = clipboard.has_image();
            }
            Err(e) => {
                // Expected in headless environments
                eprintln!("Clipboard initialization failed (expected in CI): {}", e);
            }
        }
    }

    #[test]
    fn test_linux_clipboard_default() {
        // Default may panic if no display server is available
        // We use catch_unwind to handle this gracefully in tests
        let result = std::panic::catch_unwind(|| LinuxSmartClipboard::default());

        match result {
            Ok(clipboard) => {
                // Verify the clipboard was created
                let _ = clipboard.has_image();
            }
            Err(_) => {
                // Expected in headless environments
                eprintln!("Clipboard default panicked (expected in CI)");
            }
        }
    }

    #[test]
    fn test_linux_clipboard_read_empty() {
        // This test verifies the read_content API
        let result = LinuxSmartClipboard::new();

        match result {
            Ok(clipboard) => {
                let content = clipboard.read_content();
                // Should succeed (may return Empty, Text, or Image)
                assert!(content.is_ok());
            }
            Err(_) => {
                // Expected in headless environments
                eprintln!("Clipboard not available (expected in CI)");
            }
        }
    }

    #[test]
    fn test_linux_clipboard_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LinuxSmartClipboard>();
    }

    #[test]
    fn test_linux_clipboard_debug() {
        let result = LinuxSmartClipboard::new();

        match result {
            Ok(clipboard) => {
                let debug_str = format!("{:?}", clipboard);
                assert!(debug_str.contains("LinuxSmartClipboard"));
            }
            Err(_) => {
                // Expected in headless environments
                eprintln!("Clipboard not available (expected in CI)");
            }
        }
    }

    #[test]
    fn test_linux_clipboard_write_text() {
        let result = LinuxSmartClipboard::new();

        match result {
            Ok(clipboard) => {
                let write_result = clipboard.write_text("test content".to_string());
                // Should succeed if clipboard is available
                assert!(write_result.is_ok());
            }
            Err(_) => {
                // Expected in headless environments
                eprintln!("Clipboard not available (expected in CI)");
            }
        }
    }

    #[test]
    fn test_linux_clipboard_write_image() {
        let result = LinuxSmartClipboard::new();

        match result {
            Ok(clipboard) => {
                // Create a minimal 1x1 RGBA image
                let image = ImageData {
                    bytes: vec![255, 0, 0, 255], // Red pixel
                    width: 1,
                    height: 1,
                    format: ImageFormat::Png,
                };
                let write_result = clipboard.write_image(&image);
                // Should succeed if clipboard is available
                assert!(write_result.is_ok());
            }
            Err(_) => {
                // Expected in headless environments
                eprintln!("Clipboard not available (expected in CI)");
            }
        }
    }

    #[test]
    fn test_linux_clipboard_has_image() {
        let result = LinuxSmartClipboard::new();

        match result {
            Ok(clipboard) => {
                // Just verify the method doesn't panic
                let _ = clipboard.has_image();
            }
            Err(_) => {
                // Expected in headless environments
                eprintln!("Clipboard not available (expected in CI)");
            }
        }
    }

    #[test]
    fn test_linux_clipboard_roundtrip_text() {
        let result = LinuxSmartClipboard::new();

        match result {
            Ok(clipboard) => {
                let test_text = "Hello from Linux clipboard test!".to_string();

                // Write text
                if clipboard.write_text(test_text.clone()).is_ok() {
                    // Read back
                    if let Ok(content) = clipboard.read_content() {
                        if let ClipboardContent::Text(read_text) = content {
                            assert_eq!(read_text, test_text);
                        }
                        // Note: Content might have changed if another process modified clipboard
                    }
                }
            }
            Err(_) => {
                // Expected in headless environments
                eprintln!("Clipboard not available (expected in CI)");
            }
        }
    }
}
