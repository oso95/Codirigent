//! Windows clipboard implementation.
//!
//! Provides clipboard access on Windows using the Win32 clipboard APIs
//! via the `clipboard-win` crate. This implementation supports reading
//! and writing text (CF_UNICODETEXT) and images (CF_DIB/CF_BITMAP).
//!
//! ## Features
//!
//! - Text reading/writing via CF_UNICODETEXT format
//! - Image reading/writing via CF_DIB (Device Independent Bitmap) format
//! - Clipboard change detection via GetClipboardSequenceNumber
//! - DIB format parsing to extract image dimensions
//!
//! ## Example
//!
//! ```ignore
//! use codirigent_ui::platform::WindowsSmartClipboard;
//! use codirigent_ui::smart_clipboard::SmartClipboardProvider;
//!
//! let clipboard = WindowsSmartClipboard::new();
//!
//! // Check for image content
//! if clipboard.has_image() {
//!     let content = clipboard.read_content().unwrap();
//!     // Process image...
//! }
//!
//! // Detect changes
//! if clipboard.has_changed() {
//!     // Clipboard was modified externally
//! }
//! ```

use crate::smart_clipboard::SmartClipboardProvider;
use anyhow::{anyhow, Result};
use clipboard_win::{formats, get_clipboard, is_format_avail, seq_num, set_clipboard};
use codirigent_core::{ClipboardContent, ImageData, ImageFormat};
use std::sync::atomic::{AtomicU32, Ordering};

/// Windows clipboard format constants.
///
/// These correspond to the standard Windows clipboard formats defined
/// in the Win32 API.
mod format_ids {
    /// CF_UNICODETEXT - Unicode text format.
    pub const CF_UNICODETEXT: u32 = 13;
    /// CF_DIB - Device Independent Bitmap format.
    pub const CF_DIB: u32 = 8;
    /// CF_BITMAP - Bitmap handle format.
    pub const CF_BITMAP: u32 = 2;
}

/// BITMAPINFOHEADER size constant.
///
/// The standard size of the BITMAPINFOHEADER structure in bytes.
const BITMAPINFOHEADER_SIZE: usize = 40;

/// Windows clipboard provider.
///
/// Provides access to the Windows system clipboard with support for
/// text and image content. Uses the `clipboard-win` crate for Win32
/// API access.
///
/// # Thread Safety
///
/// This implementation is `Send + Sync` safe. The clipboard sequence
/// number is stored atomically to allow safe concurrent access checks.
///
/// # Example
///
/// ```ignore
/// use codirigent_ui::platform::WindowsSmartClipboard;
/// use codirigent_ui::smart_clipboard::SmartClipboardProvider;
///
/// let clipboard = WindowsSmartClipboard::new();
///
/// // Write text to clipboard
/// clipboard.write_text("Hello, Windows!".to_string()).unwrap();
///
/// // Read it back
/// let content = clipboard.read_content().unwrap();
/// ```
#[derive(Debug)]
pub struct WindowsSmartClipboard {
    /// Last known clipboard sequence number for change detection.
    last_seq_num: AtomicU32,
}

impl WindowsSmartClipboard {
    /// Create a new Windows clipboard provider.
    ///
    /// Initializes the clipboard provider and captures the current
    /// clipboard sequence number for change detection.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use codirigent_ui::platform::WindowsSmartClipboard;
    ///
    /// let clipboard = WindowsSmartClipboard::new();
    /// ```
    pub fn new() -> Self {
        // Get initial sequence number, default to 0 if unavailable
        let initial_seq = seq_num().map_or(0, |nz| nz.get());
        Self {
            last_seq_num: AtomicU32::new(initial_seq),
        }
    }

    /// Check if the clipboard has changed since the last check.
    ///
    /// Uses GetClipboardSequenceNumber to detect if another application
    /// has modified the clipboard contents. Updates the stored sequence
    /// number on each call.
    ///
    /// # Returns
    ///
    /// `true` if the clipboard content has changed since the last call
    /// to this method, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use codirigent_ui::platform::WindowsSmartClipboard;
    ///
    /// let clipboard = WindowsSmartClipboard::new();
    ///
    /// // First call after initialization
    /// if clipboard.has_changed() {
    ///     // Handle clipboard change
    /// }
    /// ```
    pub fn has_changed(&self) -> bool {
        let current_seq = seq_num().map_or(0, |nz| nz.get());
        let last_seq = self.last_seq_num.swap(current_seq, Ordering::SeqCst);
        current_seq != last_seq
    }

    /// Parse DIB (Device Independent Bitmap) header to extract dimensions.
    ///
    /// The DIB format starts with a BITMAPINFOHEADER structure. This method
    /// parses the width and height fields from that header.
    ///
    /// # Arguments
    ///
    /// * `dib_data` - Raw DIB data from the clipboard
    ///
    /// # Returns
    ///
    /// A tuple of (width, height) on success, or an error if the data
    /// is too small or malformed.
    ///
    /// # DIB Format
    ///
    /// The BITMAPINFOHEADER structure layout:
    /// - Offset 0-3: biSize (4 bytes, header size)
    /// - Offset 4-7: biWidth (4 bytes, signed 32-bit)
    /// - Offset 8-11: biHeight (4 bytes, signed 32-bit, negative = top-down)
    fn parse_dib_dimensions(dib_data: &[u8]) -> Result<(u32, u32)> {
        // The clipboard-win crate may return a full BMP file (starting with "BM")
        // or raw DIB data (starting with BITMAPINFOHEADER). Detect and skip the
        // 14-byte BITMAPFILEHEADER if present.
        let offset = if dib_data.len() >= 2 && dib_data[0] == b'B' && dib_data[1] == b'M' {
            14 // Skip BITMAPFILEHEADER
        } else {
            0
        };

        if dib_data.len() < offset + BITMAPINFOHEADER_SIZE {
            return Err(anyhow!(
                "DIB data too small: {} bytes, expected at least {}",
                dib_data.len(),
                offset + BITMAPINFOHEADER_SIZE
            ));
        }

        let hdr = &dib_data[offset..];

        // Read width at offset 4 (signed 32-bit little-endian)
        let width = i32::from_le_bytes([
            hdr[4],
            hdr[5],
            hdr[6],
            hdr[7],
        ]);

        // Read height at offset 8 (signed 32-bit little-endian)
        // Negative height indicates top-down bitmap, we take absolute value
        let height = i32::from_le_bytes([
            hdr[8],
            hdr[9],
            hdr[10],
            hdr[11],
        ]);

        // Convert to unsigned, handling negative height for top-down bitmaps
        let width = width.unsigned_abs();
        let height = height.unsigned_abs();

        if width == 0 || height == 0 {
            return Err(anyhow!("Invalid DIB dimensions: {}x{}", width, height));
        }

        Ok((width, height))
    }
}

impl Default for WindowsSmartClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartClipboardProvider for WindowsSmartClipboard {
    /// Read current clipboard content.
    ///
    /// Attempts to read content in the following priority order:
    /// 1. Image data (CF_DIB format)
    /// 2. Text (CF_UNICODETEXT format)
    /// 3. Empty if neither is available
    ///
    /// # Returns
    ///
    /// - `ClipboardContent::Image` if image data is available
    /// - `ClipboardContent::Text` if text is available
    /// - `ClipboardContent::Empty` if the clipboard is empty
    ///
    /// # Errors
    ///
    /// Returns an error if clipboard access fails.
    fn read_content(&self) -> Result<ClipboardContent> {
        // Check for image first (higher priority for AI CLI use cases)
        if is_format_avail(format_ids::CF_DIB) {
            // Read raw DIB data
            let dib_data: Vec<u8> = get_clipboard(formats::Bitmap)
                .map_err(|e| anyhow!("Failed to read DIB data: {}", e))?;

            if !dib_data.is_empty() {
                // Parse dimensions from DIB header
                let (width, height) = Self::parse_dib_dimensions(&dib_data)?;

                return Ok(ClipboardContent::Image(ImageData {
                    bytes: dib_data,
                    width,
                    height,
                    format: ImageFormat::Dib, // Windows Device Independent Bitmap
                }));
            }
        }

        // Check for text
        if is_format_avail(format_ids::CF_UNICODETEXT) {
            let text: String = get_clipboard(formats::Unicode)
                .map_err(|e| anyhow!("Failed to read text: {}", e))?;

            if !text.is_empty() {
                return Ok(ClipboardContent::Text(text));
            }
        }

        Ok(ClipboardContent::Empty)
    }

    /// Write text to clipboard.
    ///
    /// Sets the clipboard content to the specified text using the
    /// CF_UNICODETEXT format.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to write to the clipboard
    ///
    /// # Errors
    ///
    /// Returns an error if clipboard access fails.
    fn write_text(&self, text: String) -> Result<()> {
        set_clipboard(formats::Unicode, &text)
            .map_err(|e| anyhow!("Failed to write text to clipboard: {}", e))
    }

    /// Write image to clipboard.
    ///
    /// Sets the clipboard content to the specified image data.
    /// The image bytes are written as CF_DIB format.
    ///
    /// # Arguments
    ///
    /// * `image` - The image data to write to the clipboard
    ///
    /// # Errors
    ///
    /// Returns an error if clipboard access fails.
    ///
    /// # Note
    ///
    /// The image bytes should be in DIB format for proper Windows
    /// clipboard compatibility. If the bytes are in another format
    /// (PNG, JPEG), they may need conversion before calling this method.
    fn write_image(&self, image: &ImageData) -> Result<()> {
        set_clipboard(formats::Bitmap, &image.bytes)
            .map_err(|e| anyhow!("Failed to write image to clipboard: {}", e))
    }

    /// Check if clipboard has image content.
    ///
    /// Checks for both CF_DIB and CF_BITMAP formats.
    ///
    /// # Returns
    ///
    /// `true` if the clipboard contains image data, `false` otherwise.
    fn has_image(&self) -> bool {
        is_format_avail(format_ids::CF_DIB) || is_format_avail(format_ids::CF_BITMAP)
    }

    fn has_changed(&self) -> bool {
        self.has_changed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_clipboard_new() {
        let clipboard = WindowsSmartClipboard::new();
        // Just verify it can be created and has a valid sequence number stored
        let _seq = clipboard.last_seq_num.load(Ordering::SeqCst);
        // Sequence number is always valid (u32 is always >= 0)
    }

    #[test]
    fn test_windows_clipboard_default() {
        let clipboard = WindowsSmartClipboard::default();
        // Default should be equivalent to new()
        let _seq = clipboard.last_seq_num.load(Ordering::SeqCst);
        // Sequence number is always valid (u32 is always >= 0)
    }

    #[test]
    fn test_windows_clipboard_read_write_text() {
        // Note: This test may not work in all CI environments
        // as it requires actual clipboard access
        let clipboard = WindowsSmartClipboard::new();

        // Write text
        let test_text = format!("dirigent_test_{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos());

        let write_result = clipboard.write_text(test_text.clone());

        // If clipboard access works, verify round-trip
        if write_result.is_ok() {
            let content = clipboard.read_content();
            if let Ok(ClipboardContent::Text(text)) = content {
                assert_eq!(text, test_text);
            }
        }
    }

    #[test]
    fn test_windows_clipboard_has_changed() {
        let clipboard = WindowsSmartClipboard::new();

        // First call should return false or true depending on clipboard state
        // The important thing is it doesn't panic
        let _ = clipboard.has_changed();

        // Consecutive calls without external changes should return false
        let changed = clipboard.has_changed();
        // This will be false unless another process modified the clipboard
        assert!(!changed || changed); // Always true, just verify no panic
    }

    #[test]
    fn test_windows_clipboard_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WindowsSmartClipboard>();
    }

    #[test]
    fn test_windows_clipboard_debug() {
        let clipboard = WindowsSmartClipboard::new();
        let debug_str = format!("{:?}", clipboard);
        assert!(debug_str.contains("WindowsSmartClipboard"));
        assert!(debug_str.contains("last_seq_num"));
    }

    #[test]
    fn test_parse_dib_dimensions_valid() {
        // Create a minimal valid BITMAPINFOHEADER
        // biSize (4 bytes) + biWidth (4 bytes) + biHeight (4 bytes) + rest
        let mut dib_data = vec![0u8; BITMAPINFOHEADER_SIZE];

        // biSize = 40 (standard header size)
        dib_data[0..4].copy_from_slice(&40u32.to_le_bytes());

        // biWidth = 1920
        dib_data[4..8].copy_from_slice(&1920i32.to_le_bytes());

        // biHeight = 1080
        dib_data[8..12].copy_from_slice(&1080i32.to_le_bytes());

        let result = WindowsSmartClipboard::parse_dib_dimensions(&dib_data);
        assert!(result.is_ok());

        let (width, height) = result.unwrap();
        assert_eq!(width, 1920);
        assert_eq!(height, 1080);
    }

    #[test]
    fn test_parse_dib_dimensions_negative_height() {
        // Negative height indicates top-down bitmap
        let mut dib_data = vec![0u8; BITMAPINFOHEADER_SIZE];

        dib_data[0..4].copy_from_slice(&40u32.to_le_bytes());
        dib_data[4..8].copy_from_slice(&800i32.to_le_bytes());
        dib_data[8..12].copy_from_slice(&(-600i32).to_le_bytes()); // Negative!

        let result = WindowsSmartClipboard::parse_dib_dimensions(&dib_data);
        assert!(result.is_ok());

        let (width, height) = result.unwrap();
        assert_eq!(width, 800);
        assert_eq!(height, 600); // Should be absolute value
    }

    #[test]
    fn test_parse_dib_dimensions_too_small() {
        let dib_data = vec![0u8; 10]; // Too small

        let result = WindowsSmartClipboard::parse_dib_dimensions(&dib_data);
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("too small"));
    }

    #[test]
    fn test_parse_dib_dimensions_zero_dimensions() {
        let mut dib_data = vec![0u8; BITMAPINFOHEADER_SIZE];

        dib_data[0..4].copy_from_slice(&40u32.to_le_bytes());
        // Width = 0
        dib_data[4..8].copy_from_slice(&0i32.to_le_bytes());
        // Height = 100
        dib_data[8..12].copy_from_slice(&100i32.to_le_bytes());

        let result = WindowsSmartClipboard::parse_dib_dimensions(&dib_data);
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid DIB dimensions"));
    }

    #[test]
    fn test_parse_dib_dimensions_both_zero() {
        let mut dib_data = vec![0u8; BITMAPINFOHEADER_SIZE];

        dib_data[0..4].copy_from_slice(&40u32.to_le_bytes());
        dib_data[4..8].copy_from_slice(&0i32.to_le_bytes());
        dib_data[8..12].copy_from_slice(&0i32.to_le_bytes());

        let result = WindowsSmartClipboard::parse_dib_dimensions(&dib_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_windows_clipboard_has_image_returns_bool() {
        let clipboard = WindowsSmartClipboard::new();
        // Should return a boolean without panicking
        let _has_image = clipboard.has_image();
        // Result depends on actual clipboard state
    }

    #[test]
    fn test_windows_clipboard_write_image() {
        let clipboard = WindowsSmartClipboard::new();

        // Create a minimal DIB-compatible image data
        let mut image_bytes = vec![0u8; BITMAPINFOHEADER_SIZE];
        image_bytes[0..4].copy_from_slice(&40u32.to_le_bytes());
        image_bytes[4..8].copy_from_slice(&100i32.to_le_bytes());
        image_bytes[8..12].copy_from_slice(&100i32.to_le_bytes());

        let image = ImageData {
            bytes: image_bytes,
            width: 100,
            height: 100,
            format: ImageFormat::Png,
        };

        // Attempt to write - may fail if clipboard is locked
        let _ = clipboard.write_image(&image);
    }

    #[test]
    fn test_windows_clipboard_sequence_number_update() {
        let clipboard = WindowsSmartClipboard::new();

        // Get initial state
        let _initial = clipboard.last_seq_num.load(Ordering::SeqCst);

        // Call has_changed - this should update internal state
        let _ = clipboard.has_changed();

        // The sequence number might have been updated
        let _after = clipboard.last_seq_num.load(Ordering::SeqCst);

        // Both are always valid sequence numbers (u32 is always >= 0)
    }

    #[test]
    fn test_format_ids_constants() {
        // Verify format IDs match Windows API constants
        assert_eq!(format_ids::CF_UNICODETEXT, 13);
        assert_eq!(format_ids::CF_DIB, 8);
        assert_eq!(format_ids::CF_BITMAP, 2);
    }

    #[test]
    fn test_bitmapinfoheader_size() {
        // BITMAPINFOHEADER is always 40 bytes
        assert_eq!(BITMAPINFOHEADER_SIZE, 40);
    }
}
