//! Smart clipboard provider with image support.
//!
//! This module provides a platform-abstracted clipboard interface that supports
//! text, images, and file content. It extends the basic clipboard functionality
//! with image handling capabilities needed for AI CLI integrations.
//!
//! ## Architecture
//!
//! The smart clipboard uses a trait-based design to allow platform-specific
//! implementations while maintaining a consistent interface:
//!
//! - [`SmartClipboardProvider`] - Trait defining clipboard operations
//! - [`ThumbnailPreview`] - Image preview data for UI display
//!
//! ## Platform Support
//!
//! Platform-specific implementations are provided in the `platform` module:
//! - macOS: Uses NSPasteboard APIs (MVP: stub implementation)
//! - Other platforms: Stub implementation returning empty content
//!
//! ## Example
//!
//! ```
//! use codirigent_ui::smart_clipboard::{SmartClipboardProvider, ThumbnailPreview};
//! use codirigent_core::{ClipboardContent, ImageData, ImageFormat};
//! use std::path::PathBuf;
//!
//! // Create a thumbnail preview
//! let preview = ThumbnailPreview::new(
//!     vec![0x89, 0x50, 0x4E, 0x47], // PNG magic bytes
//!     PathBuf::from("/tmp/screenshot.png"),
//!     1920,
//!     1080,
//!     1024000,
//!     ImageFormat::Png,
//! );
//! assert_eq!(preview.original_width, 1920);
//! assert_eq!(preview.original_height, 1080);
//! ```

use anyhow::Result;
use codirigent_core::{ClipboardContent, ImageData, ImageFormat};
use std::path::PathBuf;

/// Platform-specific clipboard provider with image support.
///
/// This trait extends basic clipboard functionality with image handling
/// capabilities required for AI CLI integrations. Implementations must
/// be thread-safe (`Send + Sync`) to allow usage from multiple contexts.
///
/// # Example
///
/// ```
/// use codirigent_ui::smart_clipboard::SmartClipboardProvider;
/// use codirigent_core::{ClipboardContent, ImageData};
/// use anyhow::Result;
///
/// // Example stub implementation
/// struct MyClipboard;
///
/// impl SmartClipboardProvider for MyClipboard {
///     fn read_content(&self) -> Result<ClipboardContent> {
///         Ok(ClipboardContent::Empty)
///     }
///
///     fn write_text(&self, _text: String) -> Result<()> {
///         Ok(())
///     }
///
///     fn write_image(&self, _image: &ImageData) -> Result<()> {
///         Ok(())
///     }
///
///     fn has_image(&self) -> bool {
///         false
///     }
///
///     fn has_changed(&self) -> bool {
///         false
///     }
/// }
/// ```
pub trait SmartClipboardProvider: Send + Sync {
    /// Read current clipboard content (text, image, or files).
    ///
    /// Returns the current clipboard content, detecting the appropriate
    /// type based on what's available.
    ///
    /// # Returns
    ///
    /// - `ClipboardContent::Text` if text is available
    /// - `ClipboardContent::Image` if image data is available
    /// - `ClipboardContent::Files` if file paths are available
    /// - `ClipboardContent::Empty` if the clipboard is empty or unreadable
    ///
    /// # Errors
    ///
    /// Returns an error if clipboard access fails.
    fn read_content(&self) -> Result<ClipboardContent>;

    /// Write text to clipboard.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to write to the clipboard
    ///
    /// # Errors
    ///
    /// Returns an error if clipboard access fails.
    fn write_text(&self, text: String) -> Result<()>;

    /// Write image to clipboard.
    ///
    /// # Arguments
    ///
    /// * `image` - The image data to write to the clipboard
    ///
    /// # Errors
    ///
    /// Returns an error if clipboard access fails.
    fn write_image(&self, image: &ImageData) -> Result<()>;

    /// Check if clipboard has image content.
    ///
    /// This is a lightweight check that doesn't read the full image data.
    ///
    /// # Returns
    ///
    /// `true` if the clipboard contains image data, `false` otherwise.
    fn has_image(&self) -> bool;

    /// Check if clipboard content has changed since the last call.
    ///
    /// Uses platform-specific mechanisms (e.g., clipboard sequence numbers on Windows)
    /// to detect changes. Each call updates the internal state, so consecutive calls
    /// without external changes will return `false`.
    ///
    /// # Returns
    ///
    /// `true` if the clipboard was modified since the last call, `false` otherwise.
    fn has_changed(&self) -> bool;
}

/// Thumbnail preview data for UI display.
///
/// Contains a scaled-down version of an image along with metadata
/// for displaying previews in the UI. Thumbnails are limited to
/// 128x128 pixels maximum to keep memory usage low.
///
/// # Example
///
/// ```
/// use codirigent_ui::smart_clipboard::ThumbnailPreview;
/// use codirigent_core::ImageFormat;
/// use std::path::PathBuf;
///
/// let preview = ThumbnailPreview::new(
///     vec![0x89, 0x50, 0x4E, 0x47], // PNG magic bytes (simplified)
///     PathBuf::from("/tmp/screenshot.png"),
///     1920,
///     1080,
///     2048000,
///     ImageFormat::Png,
/// );
///
/// assert_eq!(preview.original_width, 1920);
/// assert_eq!(preview.original_height, 1080);
/// assert_eq!(preview.file_size, 2048000);
/// assert_eq!(preview.format, ImageFormat::Png);
/// ```
#[derive(Debug, Clone)]
pub struct ThumbnailPreview {
    /// Scaled image bytes (128x128 max).
    pub thumbnail_bytes: Vec<u8>,
    /// Original image path.
    pub image_path: PathBuf,
    /// Original image width in pixels.
    pub original_width: u32,
    /// Original image height in pixels.
    pub original_height: u32,
    /// File size in bytes.
    pub file_size: u64,
    /// Image format of the thumbnail bytes.
    pub format: ImageFormat,
}

impl ThumbnailPreview {
    /// Create a new thumbnail preview.
    ///
    /// # Arguments
    ///
    /// * `thumbnail_bytes` - Scaled image bytes (should be 128x128 max)
    /// * `image_path` - Path to the original image file
    /// * `original_width` - Original image width in pixels
    /// * `original_height` - Original image height in pixels
    /// * `file_size` - Size of the original file in bytes
    /// * `format` - Image format of the thumbnail bytes
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::smart_clipboard::ThumbnailPreview;
    /// use codirigent_core::ImageFormat;
    /// use std::path::PathBuf;
    ///
    /// let preview = ThumbnailPreview::new(
    ///     vec![1, 2, 3, 4],
    ///     PathBuf::from("/tmp/image.png"),
    ///     800,
    ///     600,
    ///     512000,
    ///     ImageFormat::Png,
    /// );
    /// assert_eq!(preview.image_path, PathBuf::from("/tmp/image.png"));
    /// assert_eq!(preview.format, ImageFormat::Png);
    /// ```
    pub fn new(
        thumbnail_bytes: Vec<u8>,
        image_path: PathBuf,
        original_width: u32,
        original_height: u32,
        file_size: u64,
        format: ImageFormat,
    ) -> Self {
        Self {
            thumbnail_bytes,
            image_path,
            original_width,
            original_height,
            file_size,
            format,
        }
    }

    /// Get the aspect ratio of the original image.
    ///
    /// # Returns
    ///
    /// The aspect ratio as width / height. Returns 1.0 if height is zero.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::smart_clipboard::ThumbnailPreview;
    /// use codirigent_core::ImageFormat;
    /// use std::path::PathBuf;
    ///
    /// let preview = ThumbnailPreview::new(
    ///     vec![],
    ///     PathBuf::from("/tmp/image.png"),
    ///     1920,
    ///     1080,
    ///     0,
    ///     ImageFormat::Png,
    /// );
    /// let ratio = preview.aspect_ratio();
    /// assert!((ratio - 1.777).abs() < 0.01); // ~16:9
    /// ```
    pub fn aspect_ratio(&self) -> f64 {
        if self.original_height == 0 {
            1.0
        } else {
            self.original_width as f64 / self.original_height as f64
        }
    }

    /// Check if the thumbnail data is empty.
    ///
    /// # Returns
    ///
    /// `true` if thumbnail_bytes is empty, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::smart_clipboard::ThumbnailPreview;
    /// use codirigent_core::ImageFormat;
    /// use std::path::PathBuf;
    ///
    /// let empty_preview = ThumbnailPreview::new(
    ///     vec![],
    ///     PathBuf::from("/tmp/image.png"),
    ///     100,
    ///     100,
    ///     0,
    ///     ImageFormat::Png,
    /// );
    /// assert!(empty_preview.is_empty());
    ///
    /// let filled_preview = ThumbnailPreview::new(
    ///     vec![1, 2, 3],
    ///     PathBuf::from("/tmp/image.png"),
    ///     100,
    ///     100,
    ///     100,
    ///     ImageFormat::Png,
    /// );
    /// assert!(!filled_preview.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.thumbnail_bytes.is_empty()
    }

    /// Get a human-readable file size string.
    ///
    /// # Returns
    ///
    /// A formatted string like "1.5 MB" or "256 KB".
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::smart_clipboard::ThumbnailPreview;
    /// use codirigent_core::ImageFormat;
    /// use std::path::PathBuf;
    ///
    /// let preview = ThumbnailPreview::new(
    ///     vec![],
    ///     PathBuf::from("/tmp/image.png"),
    ///     100,
    ///     100,
    ///     1536000, // 1.5 MB
    ///     ImageFormat::Png,
    /// );
    /// assert_eq!(preview.human_readable_size(), "1.5 MB");
    /// ```
    pub fn human_readable_size(&self) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;

        if self.file_size >= GB {
            format!("{:.1} GB", self.file_size as f64 / GB as f64)
        } else if self.file_size >= MB {
            format!("{:.1} MB", self.file_size as f64 / MB as f64)
        } else if self.file_size >= KB {
            format!("{:.1} KB", self.file_size as f64 / KB as f64)
        } else {
            format!("{} B", self.file_size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ThumbnailPreview tests

    #[test]
    fn test_thumbnail_preview_new() {
        let bytes = vec![0x89, 0x50, 0x4E, 0x47];
        let path = PathBuf::from("/tmp/screenshot.png");
        let preview =
            ThumbnailPreview::new(bytes.clone(), path.clone(), 1920, 1080, 1024000, ImageFormat::Png);

        assert_eq!(preview.thumbnail_bytes, bytes);
        assert_eq!(preview.image_path, path);
        assert_eq!(preview.original_width, 1920);
        assert_eq!(preview.original_height, 1080);
        assert_eq!(preview.file_size, 1024000);
        assert_eq!(preview.format, ImageFormat::Png);
    }

    #[test]
    fn test_thumbnail_preview_aspect_ratio() {
        // 16:9 aspect ratio
        let preview = ThumbnailPreview::new(vec![], PathBuf::new(), 1920, 1080, 0, ImageFormat::Png);
        let ratio = preview.aspect_ratio();
        assert!((ratio - 1.777).abs() < 0.01);

        // 4:3 aspect ratio
        let preview = ThumbnailPreview::new(vec![], PathBuf::new(), 800, 600, 0, ImageFormat::Png);
        let ratio = preview.aspect_ratio();
        assert!((ratio - 1.333).abs() < 0.01);

        // 1:1 aspect ratio
        let preview = ThumbnailPreview::new(vec![], PathBuf::new(), 100, 100, 0, ImageFormat::Png);
        let ratio = preview.aspect_ratio();
        assert!((ratio - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_thumbnail_preview_aspect_ratio_zero_height() {
        let preview = ThumbnailPreview::new(vec![], PathBuf::new(), 100, 0, 0, ImageFormat::Png);
        assert_eq!(preview.aspect_ratio(), 1.0);
    }

    #[test]
    fn test_thumbnail_preview_is_empty() {
        let empty = ThumbnailPreview::new(vec![], PathBuf::new(), 100, 100, 0, ImageFormat::Png);
        assert!(empty.is_empty());

        let filled = ThumbnailPreview::new(vec![1, 2, 3], PathBuf::new(), 100, 100, 100, ImageFormat::Png);
        assert!(!filled.is_empty());
    }

    #[test]
    fn test_thumbnail_preview_human_readable_size_bytes() {
        let preview = ThumbnailPreview::new(vec![], PathBuf::new(), 0, 0, 512, ImageFormat::Png);
        assert_eq!(preview.human_readable_size(), "512 B");
    }

    #[test]
    fn test_thumbnail_preview_human_readable_size_kb() {
        let preview = ThumbnailPreview::new(vec![], PathBuf::new(), 0, 0, 1536, ImageFormat::Png);
        assert_eq!(preview.human_readable_size(), "1.5 KB");
    }

    #[test]
    fn test_thumbnail_preview_human_readable_size_mb() {
        let preview = ThumbnailPreview::new(vec![], PathBuf::new(), 0, 0, 1536000, ImageFormat::Png);
        assert_eq!(preview.human_readable_size(), "1.5 MB");
    }

    #[test]
    fn test_thumbnail_preview_human_readable_size_gb() {
        let preview = ThumbnailPreview::new(vec![], PathBuf::new(), 0, 0, 1610612736, ImageFormat::Png);
        assert_eq!(preview.human_readable_size(), "1.5 GB");
    }

    #[test]
    fn test_thumbnail_preview_clone() {
        let original = ThumbnailPreview::new(
            vec![1, 2, 3],
            PathBuf::from("/tmp/test.png"),
            800,
            600,
            1000,
            ImageFormat::Png,
        );
        let cloned = original.clone();

        assert_eq!(cloned.thumbnail_bytes, original.thumbnail_bytes);
        assert_eq!(cloned.image_path, original.image_path);
        assert_eq!(cloned.original_width, original.original_width);
        assert_eq!(cloned.original_height, original.original_height);
        assert_eq!(cloned.file_size, original.file_size);
        assert_eq!(cloned.format, original.format);
    }

    #[test]
    fn test_thumbnail_preview_debug() {
        let preview = ThumbnailPreview::new(
            vec![1, 2, 3],
            PathBuf::from("/tmp/test.png"),
            800,
            600,
            1000,
            ImageFormat::Png,
        );
        let debug_str = format!("{:?}", preview);
        assert!(debug_str.contains("ThumbnailPreview"));
        assert!(debug_str.contains("original_width: 800"));
        assert!(debug_str.contains("original_height: 600"));
    }

    // SmartClipboardProvider compile-time test
    // This test ensures the trait has proper bounds (Send + Sync)

    #[test]
    fn test_smart_clipboard_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        // This is a compile-time check - if it compiles, the test passes
        // We use a concrete implementation to verify the trait bounds
        fn check_trait_bounds<T: SmartClipboardProvider>() {
            assert_send_sync::<T>();
        }

        // The fact that this compiles proves SmartClipboardProvider requires Send + Sync
        let _ = check_trait_bounds::<crate::platform::StubSmartClipboard>;
    }
}
