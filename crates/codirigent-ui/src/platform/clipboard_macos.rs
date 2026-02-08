//! macOS clipboard implementation.
//!
//! Provides clipboard access on macOS using the NSPasteboard APIs via objc2.
//! Supports reading and writing text, images, and file URLs.
//!
//! ## Features
//!
//! - Text clipboard operations via `NSPasteboardTypeString`
//! - Image detection and reading (PNG, TIFF formats)
//! - File URL detection via `NSPasteboardTypeFileURL`
//! - Change detection via `changeCount`
//!
//! ## Thread Safety
//!
//! NSPasteboard must only be accessed from the main thread. This implementation
//! uses the `dispatch` crate to ensure all clipboard operations execute on the
//! main queue, making it safe to call from any thread.
//!
//! See: <https://wadetregaskis.com/nspasteboard-crashes-due-to-unsafe-internal-concurrent-memory-mutation-when-handling-file-promises/>

use crate::smart_clipboard::SmartClipboardProvider;
use anyhow::{anyhow, Result};
use codirigent_core::{ClipboardContent, ImageData, ImageFormat};
use objc2::rc::Retained;
use objc2_app_kit::NSPasteboard;
use objc2_foundation::{NSData, NSString};
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};
#[cfg(test)]
use std::sync::Mutex;

/// Global mutex for serializing clipboard access in test environments.
///
/// NSPasteboard is not thread-safe and ideally should only be accessed from the main thread.
/// In a GUI application with a main run loop, we use `dispatch::Queue::main().exec_sync()`.
/// In test environments (no run loop), we fall back to mutex-based serialization.
#[cfg(test)]
static CLIPBOARD_MUTEX: Mutex<()> = Mutex::new(());

/// Execute a closure with clipboard access serialization.
///
/// In production with a main run loop, dispatches to the main queue.
/// In test environments, uses a mutex for serialization.
/// If already on the main thread, calls the closure directly to avoid deadlock.
fn with_clipboard_access<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    // In test environments, there's no main run loop, so exec_sync would deadlock.
    // Use mutex serialization instead.
    #[cfg(test)]
    {
        let _guard = CLIPBOARD_MUTEX.lock().unwrap();
        f()
    }

    // In production, dispatch to the main queue for true thread safety.
    // If already on the main thread, call directly to avoid deadlock.
    #[cfg(not(test))]
    {
        if pthread_main_np() != 0 {
            f()
        } else {
            dispatch::Queue::main().exec_sync(f)
        }
    }
}

/// Returns non-zero if the current thread is the main thread.
#[cfg(not(test))]
fn pthread_main_np() -> i32 {
    extern "C" {
        fn pthread_main_np() -> i32;
    }
    unsafe { pthread_main_np() }
}

/// macOS clipboard provider.
///
/// Provides access to the macOS system clipboard with support for
/// text, images, and file paths using NSPasteboard APIs.
///
/// # Thread Safety
///
/// This type is `Send + Sync` safe. All NSPasteboard operations are dispatched
/// to the main thread using the `dispatch` crate, ensuring thread safety even
/// when called from background threads.
///
/// # Example
///
/// ```no_run
/// use codirigent_ui::platform::MacOSSmartClipboard;
/// use codirigent_ui::smart_clipboard::SmartClipboardProvider;
///
/// let clipboard = MacOSSmartClipboard::new();
///
/// // Check for image content
/// if clipboard.has_image() {
///     let content = clipboard.read_content().unwrap();
///     // Process image...
/// }
/// ```
#[derive(Debug)]
pub struct MacOSSmartClipboard {
    /// Last known change count for detecting clipboard changes.
    last_change_count: AtomicIsize,
}

impl MacOSSmartClipboard {
    /// Create a new macOS clipboard provider.
    ///
    /// Initializes the clipboard provider and captures the current
    /// change count for later change detection.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_ui::platform::MacOSSmartClipboard;
    ///
    /// let clipboard = MacOSSmartClipboard::new();
    /// ```
    pub fn new() -> Self {
        let initial_count = Self::get_current_change_count();
        Self {
            last_change_count: AtomicIsize::new(initial_count),
        }
    }

    /// Check if the clipboard has changed since the last check.
    ///
    /// Uses NSPasteboard's `changeCount` to detect changes. The change count
    /// is incremented each time the clipboard content is modified.
    ///
    /// # Returns
    ///
    /// `true` if the clipboard has changed since the last call to this method
    /// or since the clipboard was created, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_ui::platform::MacOSSmartClipboard;
    ///
    /// let clipboard = MacOSSmartClipboard::new();
    ///
    /// // First call after creation may or may not detect a change
    /// // depending on timing
    /// let changed = clipboard.has_changed();
    /// ```
    pub fn has_changed(&self) -> bool {
        let current_count = Self::get_current_change_count();
        let last_count = self.last_change_count.swap(current_count, Ordering::SeqCst);
        current_count != last_count
    }

    /// Get the current change count from the general pasteboard.
    ///
    /// Thread-safe via `with_clipboard_access`.
    fn get_current_change_count() -> isize {
        with_clipboard_access(|| {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.changeCount()
        })
    }

    /// Check if the clipboard contains a specific type.
    ///
    /// Thread-safe via `with_clipboard_access`.
    fn has_type(type_string: &NSString) -> bool {
        // We need to convert to owned string to avoid sending reference across threads
        let type_str = type_string.to_string();
        with_clipboard_access(move || {
            let pasteboard = NSPasteboard::generalPasteboard();
            let type_ns = NSString::from_str(&type_str);
            let types = pasteboard.types();
            if let Some(types) = types {
                types.containsObject(&type_ns)
            } else {
                false
            }
        })
    }

    /// Read string data from the clipboard for a given type.
    ///
    /// Thread-safe via `with_clipboard_access`.
    fn read_string_for_type(type_string: &NSString) -> Option<String> {
        let type_str = type_string.to_string();
        with_clipboard_access(move || {
            let pasteboard = NSPasteboard::generalPasteboard();
            let type_ns = NSString::from_str(&type_str);
            let ns_string = pasteboard.stringForType(&type_ns)?;
            Some(ns_string.to_string())
        })
    }

    /// Read raw data from the clipboard for a given type.
    ///
    /// Thread-safe via `with_clipboard_access`.
    fn read_data_for_type(type_string: &NSString) -> Option<Vec<u8>> {
        let type_str = type_string.to_string();
        with_clipboard_access(move || {
            let pasteboard = NSPasteboard::generalPasteboard();
            let type_ns = NSString::from_str(&type_str);
            let ns_data = pasteboard.dataForType(&type_ns)?;
            Some(ns_data.to_vec())
        })
    }

    /// Get the pasteboard type string for plain text.
    fn string_type() -> Retained<NSString> {
        NSString::from_str("public.utf8-plain-text")
    }

    /// Get the pasteboard type string for PNG images.
    fn png_type() -> Retained<NSString> {
        NSString::from_str("public.png")
    }

    /// Get the pasteboard type string for TIFF images.
    fn tiff_type() -> Retained<NSString> {
        NSString::from_str("public.tiff")
    }

    /// Get the pasteboard type string for file URLs.
    fn file_url_type() -> Retained<NSString> {
        NSString::from_str("public.file-url")
    }

    /// Parse PNG header to extract image dimensions.
    ///
    /// PNG format: 8-byte signature, then IHDR chunk with width/height.
    /// Width is at bytes 16-19, height at bytes 20-23 (big-endian).
    fn parse_png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
        // PNG signature (8 bytes) + IHDR chunk header (8 bytes) + width (4) + height (4) = 24 bytes minimum
        if data.len() < 24 {
            return None;
        }

        // Check PNG signature
        let png_signature: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        if &data[0..8] != &png_signature {
            return None;
        }

        // Width at offset 16 (big-endian)
        let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        // Height at offset 20 (big-endian)
        let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);

        Some((width, height))
    }

    /// Parse JPEG header to extract image dimensions.
    ///
    /// JPEG uses markers; we look for SOF0/SOF2 markers which contain dimensions.
    ///
    /// Note: Currently unused but kept for potential future JPEG clipboard support.
    #[allow(dead_code)]
    fn parse_jpeg_dimensions(data: &[u8]) -> Option<(u32, u32)> {
        // JPEG must start with FFD8
        if data.len() < 2 || data[0] != 0xFF || data[1] != 0xD8 {
            return None;
        }

        let mut i = 2;
        while i + 1 < data.len() {
            if data[i] != 0xFF {
                i += 1;
                continue;
            }

            let marker = data[i + 1];

            // SOF markers (Start of Frame) contain dimensions
            // SOF0 = 0xC0, SOF1 = 0xC1, SOF2 = 0xC2
            if marker == 0xC0 || marker == 0xC1 || marker == 0xC2 {
                // SOF structure: FF C0 length(2) precision(1) height(2) width(2)
                if i + 9 < data.len() {
                    let height = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                    let width = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                    return Some((width, height));
                }
                return None;
            }

            // Skip to next marker
            if i + 3 < data.len() {
                let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
                i += 2 + len;
            } else {
                break;
            }
        }

        None
    }

    /// Try to read image data from the clipboard.
    fn read_image_data(&self) -> Option<ImageData> {
        // Try PNG first
        let png_type = Self::png_type();
        if Self::has_type(&png_type) {
            if let Some(data) = Self::read_data_for_type(&png_type) {
                let (width, height) = Self::parse_png_dimensions(&data).unwrap_or((0, 0));
                return Some(ImageData {
                    bytes: data,
                    width,
                    height,
                    format: ImageFormat::Png,
                });
            }
        }

        // Try TIFF (often used by macOS screenshots)
        let tiff_type = Self::tiff_type();
        if Self::has_type(&tiff_type) {
            if let Some(data) = Self::read_data_for_type(&tiff_type) {
                // TIFF parsing is complex; return with zero dimensions
                // In a full implementation, we'd parse the TIFF header
                return Some(ImageData {
                    bytes: data,
                    width: 0,
                    height: 0,
                    format: ImageFormat::Tiff,
                });
            }
        }

        None
    }

    /// Try to read file URLs from the clipboard.
    fn read_file_urls(&self) -> Option<Vec<PathBuf>> {
        let file_url_type = Self::file_url_type();
        if !Self::has_type(&file_url_type) {
            return None;
        }

        // Read the file URL as a string and parse it
        if let Some(url_string) = Self::read_string_for_type(&file_url_type) {
            // file:// URL to path
            if let Some(path) = url_string.strip_prefix("file://") {
                // URL decode the path
                let decoded = urlencoding_decode(path);
                return Some(vec![PathBuf::from(decoded)]);
            }
        }

        None
    }
}

/// Simple URL decoding for file paths.
/// Handles %XX escape sequences correctly for UTF-8 multi-byte characters.
fn urlencoding_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Try to read two hex digits
            let mut hex = String::with_capacity(2);
            for _ in 0..2 {
                if let Some(&next) = chars.peek() {
                    if next.is_ascii_hexdigit() {
                        hex.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
            }
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    bytes.push(byte);
                    continue;
                }
            }
            // Failed to decode, keep the original
            bytes.push(b'%');
            bytes.extend(hex.as_bytes());
        } else {
            // Push UTF-8 bytes for this character
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf);
            bytes.extend(encoded.as_bytes());
        }
    }

    // Convert bytes to UTF-8 string, replacing invalid sequences
    String::from_utf8_lossy(&bytes).into_owned()
}

impl Default for MacOSSmartClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartClipboardProvider for MacOSSmartClipboard {
    /// Read current clipboard content.
    ///
    /// Checks for content in the following order:
    /// 1. Image data (PNG, TIFF)
    /// 2. File URLs
    /// 3. Plain text
    ///
    /// Returns `ClipboardContent::Empty` if none of the above are found.
    fn read_content(&self) -> Result<ClipboardContent> {
        // Check for image content first
        if let Some(image_data) = self.read_image_data() {
            return Ok(ClipboardContent::Image(image_data));
        }

        // Check for file URLs
        if let Some(files) = self.read_file_urls() {
            return Ok(ClipboardContent::Files(files));
        }

        // Check for text content
        let string_type = Self::string_type();
        if let Some(text) = Self::read_string_for_type(&string_type) {
            if !text.is_empty() {
                return Ok(ClipboardContent::Text(text));
            }
        }

        Ok(ClipboardContent::Empty)
    }

    /// Write text to clipboard.
    ///
    /// Clears the clipboard and writes the text as UTF-8 plain text.
    /// Thread-safe via `with_clipboard_access`.
    fn write_text(&self, text: String) -> Result<()> {
        with_clipboard_access(move || {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.clearContents();

            let ns_string = NSString::from_str(&text);
            let type_string = NSString::from_str("public.utf8-plain-text");

            let success = pasteboard.setString_forType(&ns_string, &type_string);
            if success {
                Ok(())
            } else {
                Err(anyhow!("Failed to write text to clipboard"))
            }
        })
    }

    /// Write image to clipboard.
    ///
    /// Clears the clipboard and writes the image data as PNG.
    /// Thread-safe via `with_clipboard_access`.
    fn write_image(&self, image: &ImageData) -> Result<()> {
        let image_bytes = image.bytes.clone();
        with_clipboard_access(move || {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.clearContents();

            // Create NSData from the image bytes
            let ns_data = NSData::with_bytes(&image_bytes);
            let type_string = NSString::from_str("public.png");

            let success = pasteboard.setData_forType(Some(&ns_data), &type_string);
            if success {
                Ok(())
            } else {
                Err(anyhow!("Failed to write image to clipboard"))
            }
        })
    }

    /// Check if clipboard has image content.
    ///
    /// Returns `true` if the clipboard contains PNG or TIFF image data.
    fn has_image(&self) -> bool {
        Self::has_type(&Self::png_type()) || Self::has_type(&Self::tiff_type())
    }

    fn has_changed(&self) -> bool {
        self.has_changed()
    }
}

// SAFETY: MacOSSmartClipboard only contains an AtomicIsize which is Send + Sync.
// All NSPasteboard operations are dispatched to the main thread via dispatch::Queue::main(),
// ensuring thread safety. We don't hold any Objective-C object references.
unsafe impl Send for MacOSSmartClipboard {}
unsafe impl Sync for MacOSSmartClipboard {}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // Tests that access the system clipboard must run serially to avoid race conditions.
    // The system clipboard is a shared global resource.

    #[test]
    #[serial(clipboard)]
    fn test_macos_clipboard_new() {
        let clipboard = MacOSSmartClipboard::new();
        // Just verify it can be created and the change count is captured
        let _ = clipboard.last_change_count.load(Ordering::SeqCst);
    }

    #[test]
    #[serial(clipboard)]
    fn test_macos_clipboard_default() {
        let clipboard = MacOSSmartClipboard::default();
        // Default should be equivalent to new()
        let _ = clipboard.last_change_count.load(Ordering::SeqCst);
    }

    #[test]
    #[serial(clipboard)]
    fn test_macos_clipboard_read_text() {
        let clipboard = MacOSSmartClipboard::new();
        let test_text = "serial_test_read_text";
        clipboard.write_text(test_text.to_string()).unwrap();

        // With serial execution, we can now reliably read back what we wrote
        let content = clipboard.read_content().unwrap();
        match content {
            ClipboardContent::Text(text) => {
                assert_eq!(text, test_text);
            }
            _ => panic!("Expected text content on clipboard"),
        }
    }

    #[test]
    #[serial(clipboard)]
    fn test_macos_clipboard_write_text() {
        let clipboard = MacOSSmartClipboard::new();
        let test_text = "serial_test_write_text";
        let result = clipboard.write_text(test_text.to_string());
        assert!(result.is_ok());

        // With serial execution, we can verify the exact content
        let content = clipboard.read_content().unwrap();
        assert!(matches!(content, ClipboardContent::Text(t) if t == test_text));
    }

    #[test]
    #[serial(clipboard)]
    fn test_macos_clipboard_has_changed() {
        let clipboard = MacOSSmartClipboard::new();

        // First check captures current state
        let _first_check = clipboard.has_changed();

        // Write to clipboard
        clipboard.write_text("change test".to_string()).unwrap();

        // Should detect change
        let changed = clipboard.has_changed();
        assert!(changed);

        // Second check without writing should show no change
        let changed_again = clipboard.has_changed();
        assert!(!changed_again);
    }

    #[test]
    #[serial(clipboard)]
    fn test_macos_clipboard_has_image() {
        let clipboard = MacOSSmartClipboard::new();

        // Write an image and verify has_image returns true
        let png_data = create_minimal_png();
        let image = ImageData {
            bytes: png_data,
            width: 8,
            height: 8,
            format: ImageFormat::Png,
        };
        clipboard.write_image(&image).unwrap();
        assert!(clipboard.has_image());

        // Write text to clear and verify has_image returns false
        clipboard.write_text("no image test".to_string()).unwrap();
        assert!(!clipboard.has_image());
    }

    #[test]
    #[serial(clipboard)]
    fn test_macos_clipboard_write_image() {
        let clipboard = MacOSSmartClipboard::new();

        // Create a minimal valid PNG (8x8 pixels, all black)
        // PNG header + IHDR chunk + IDAT chunk (minimal) + IEND chunk
        let png_data = create_minimal_png();

        let image = ImageData {
            bytes: png_data,
            width: 8,
            height: 8,
            format: ImageFormat::Png,
        };
        let result = clipboard.write_image(&image);
        assert!(result.is_ok());

        // Verify image is on clipboard
        assert!(clipboard.has_image());
    }

    #[test]
    #[serial(clipboard)]
    fn test_macos_clipboard_debug() {
        let clipboard = MacOSSmartClipboard::new();
        let debug_str = format!("{:?}", clipboard);
        assert!(debug_str.contains("MacOSSmartClipboard"));
        assert!(debug_str.contains("last_change_count"));
    }

    #[test]
    fn test_macos_clipboard_is_send_sync() {
        // This is a compile-time check, no clipboard access needed
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MacOSSmartClipboard>();
    }

    #[test]
    fn test_parse_png_dimensions() {
        // Valid PNG header with 100x200 dimensions
        let mut data = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, // IHDR length
            0x49, 0x48, 0x44, 0x52, // "IHDR"
            0x00, 0x00, 0x00, 0x64, // Width: 100 (big-endian)
            0x00, 0x00, 0x00, 0xC8, // Height: 200 (big-endian)
        ];
        // Add some padding to make it valid length
        data.extend_from_slice(&[0x08, 0x02, 0x00, 0x00, 0x00]);

        let dims = MacOSSmartClipboard::parse_png_dimensions(&data);
        assert_eq!(dims, Some((100, 200)));
    }

    #[test]
    fn test_parse_png_dimensions_invalid() {
        // Too short
        let short_data = vec![0x89, 0x50, 0x4E, 0x47];
        assert_eq!(MacOSSmartClipboard::parse_png_dimensions(&short_data), None);

        // Wrong signature
        let wrong_sig = vec![0x00; 24];
        assert_eq!(MacOSSmartClipboard::parse_png_dimensions(&wrong_sig), None);
    }

    #[test]
    fn test_parse_jpeg_dimensions() {
        // Minimal JPEG with SOF0 marker indicating 320x240
        let data = vec![
            0xFF, 0xD8, // SOI
            0xFF, 0xC0, // SOF0
            0x00, 0x11, // Length: 17
            0x08, // Precision: 8 bits
            0x00, 0xF0, // Height: 240
            0x01, 0x40, // Width: 320
            0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01,
        ];

        let dims = MacOSSmartClipboard::parse_jpeg_dimensions(&data);
        assert_eq!(dims, Some((320, 240)));
    }

    #[test]
    fn test_parse_jpeg_dimensions_invalid() {
        // Too short
        let short_data = vec![0xFF];
        assert_eq!(
            MacOSSmartClipboard::parse_jpeg_dimensions(&short_data),
            None
        );

        // Wrong signature
        let wrong_sig = vec![0x00, 0x00];
        assert_eq!(
            MacOSSmartClipboard::parse_jpeg_dimensions(&wrong_sig),
            None
        );
    }

    #[test]
    fn test_urlencoding_decode() {
        // Basic path (no encoding)
        assert_eq!(urlencoding_decode("/tmp/file.txt"), "/tmp/file.txt");

        // Encoded space
        assert_eq!(urlencoding_decode("/tmp/my%20file.txt"), "/tmp/my file.txt");

        // Multiple encoded characters
        assert_eq!(
            urlencoding_decode("/tmp/%48%65%6C%6C%6F"),
            "/tmp/Hello"
        );

        // Incomplete encoding (should preserve)
        assert_eq!(urlencoding_decode("/tmp/file%2"), "/tmp/file%2");

        // Invalid hex (should preserve)
        assert_eq!(urlencoding_decode("/tmp/file%ZZ"), "/tmp/file%ZZ");
    }

    /// Create a minimal valid PNG for testing.
    fn create_minimal_png() -> Vec<u8> {
        // 8x8 pixel PNG, all black
        let mut png = Vec::new();

        // PNG signature
        png.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

        // IHDR chunk
        let ihdr_data = [
            0x00, 0x00, 0x00, 0x08, // Width: 8
            0x00, 0x00, 0x00, 0x08, // Height: 8
            0x08, // Bit depth: 8
            0x02, // Color type: RGB
            0x00, // Compression: deflate
            0x00, // Filter: adaptive
            0x00, // Interlace: none
        ];
        png.extend_from_slice(&(ihdr_data.len() as u32).to_be_bytes()); // Length
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&ihdr_data);
        let mut ihdr_crc_data = Vec::with_capacity(4 + ihdr_data.len());
        ihdr_crc_data.extend_from_slice(b"IHDR");
        ihdr_crc_data.extend_from_slice(&ihdr_data);
        png.extend_from_slice(&crc32(&ihdr_crc_data).to_be_bytes());

        // Minimal IDAT chunk (compressed empty image data)
        // This is a minimal zlib stream for an 8x8 black image
        let idat_data = [
            0x78, 0x9C, // zlib header
            0x62, 0x60, 0x60, 0x60, // compressed data (minimal)
            0x00, 0x00, 0x00, 0x01, // adler32
        ];
        png.extend_from_slice(&(idat_data.len() as u32).to_be_bytes());
        png.extend_from_slice(b"IDAT");
        png.extend_from_slice(&idat_data);
        let mut idat_crc_data = Vec::with_capacity(4 + idat_data.len());
        idat_crc_data.extend_from_slice(b"IDAT");
        idat_crc_data.extend_from_slice(&idat_data);
        png.extend_from_slice(&crc32(&idat_crc_data).to_be_bytes());

        // IEND chunk
        png.extend_from_slice(&0u32.to_be_bytes()); // Length: 0
        png.extend_from_slice(b"IEND");
        png.extend_from_slice(&crc32(b"IEND").to_be_bytes());

        png
    }

    /// Simple CRC32 implementation for PNG chunks (for testing only).
    fn crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFFFFFF;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }
}
