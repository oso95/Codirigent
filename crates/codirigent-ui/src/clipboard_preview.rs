//! Clipboard preview UI component.
//!
//! Provides a GPUI-based thumbnail preview component for displaying image
//! clipboard content. The component shows a 128x128 thumbnail along with
//! image metadata (dimensions, file size, path).
//!
//! ## Architecture
//!
//! The `ClipboardPreview` component manages preview state and provides
//! methods for showing/hiding previews, generating thumbnails, and creating
//! preview data from images.
//!
//! ## Example
//!
//! ```
//! use codirigent_ui::clipboard_preview::ClipboardPreview;
//! use codirigent_ui::smart_clipboard::ThumbnailPreview;
//! use codirigent_ui::theme::CodirigentTheme;
//! use std::path::PathBuf;
//!
//! // Create a preview component
//! let mut preview = ClipboardPreview::new(CodirigentTheme::dark());
//!
//! // Show a preview
//! use codirigent_core::ImageFormat;
//! let thumbnail = ThumbnailPreview::new(
//!     vec![0x89, 0x50, 0x4E, 0x47],
//!     PathBuf::from("/tmp/image.png"),
//!     1920,
//!     1080,
//!     2048000,
//!     ImageFormat::Png,
//! );
//! preview.show(thumbnail);
//! assert!(preview.is_visible());
//!
//! // Hide the preview
//! preview.hide();
//! assert!(!preview.is_visible());
//! ```

use crate::smart_clipboard::ThumbnailPreview;
use crate::theme::CodirigentTheme;
use codirigent_core::{ImageData, ImageFormat as CoreImageFormat};
use image::{imageops::FilterType, ImageReader};
use std::io::Cursor;
use std::path::PathBuf;
use tracing::warn;

#[cfg(feature = "gpui-full")]
use std::sync::Arc;

#[cfg(feature = "gpui-full")]
use gpui::{
    div, px, Context, Image, ImageFormat, IntoElement, ObjectFit, ParentElement, Render, Styled,
    StyledImage, Window,
};

/// Wrapper for cached GPUI Image that implements Debug and Clone.
///
/// This wrapper allows `ClipboardPreview` to cache the GPUI Image while
/// still deriving Debug and Clone. On clone, the cache is NOT cloned
/// (each instance gets its own empty cache) to avoid sharing mutable state.
#[cfg(feature = "gpui-full")]
#[derive(Default)]
struct CachedImage(Option<Arc<Image>>);

#[cfg(feature = "gpui-full")]
impl std::fmt::Debug for CachedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedImage")
            .field("is_cached", &self.0.is_some())
            .finish()
    }
}

#[cfg(feature = "gpui-full")]
impl Clone for CachedImage {
    fn clone(&self) -> Self {
        // Don't clone the cache - each instance builds its own
        Self(None)
    }
}

/// Maximum thumbnail dimension (width or height) in pixels.
pub const MAX_THUMBNAIL_SIZE: u32 = 128;

/// Clipboard preview UI component.
///
/// Manages the display of image previews from the clipboard. Shows a
/// scaled-down thumbnail (128x128 max) along with metadata about the
/// original image.
///
/// # Example
///
/// ```
/// use codirigent_ui::clipboard_preview::ClipboardPreview;
/// use codirigent_ui::theme::CodirigentTheme;
///
/// let preview = ClipboardPreview::new(CodirigentTheme::dark());
/// assert!(!preview.is_visible());
/// ```
#[derive(Debug, Clone)]
pub struct ClipboardPreview {
    /// Theme for rendering.
    theme: CodirigentTheme,
    /// Current preview data (if visible).
    current_preview: Option<ThumbnailPreview>,
    /// Whether the preview is visible.
    visible: bool,
    /// Cached GPUI image to avoid recreating on every render cycle.
    #[cfg(feature = "gpui-full")]
    cached_image: CachedImage,
}

impl ClipboardPreview {
    /// Create a new clipboard preview component.
    ///
    /// # Arguments
    ///
    /// * `theme` - The theme to use for rendering
    ///
    /// # Returns
    ///
    /// A new `ClipboardPreview` instance with no preview showing.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::clipboard_preview::ClipboardPreview;
    /// use codirigent_ui::theme::CodirigentTheme;
    ///
    /// let preview = ClipboardPreview::new(CodirigentTheme::dark());
    /// assert!(!preview.is_visible());
    /// assert!(preview.preview().is_none());
    /// ```
    pub fn new(theme: CodirigentTheme) -> Self {
        Self {
            theme,
            current_preview: None,
            visible: false,
            #[cfg(feature = "gpui-full")]
            cached_image: CachedImage::default(),
        }
    }

    /// Show a preview with the given data.
    ///
    /// # Arguments
    ///
    /// * `preview` - The thumbnail preview data to display
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::clipboard_preview::ClipboardPreview;
    /// use codirigent_ui::smart_clipboard::ThumbnailPreview;
    /// use codirigent_ui::theme::CodirigentTheme;
    /// use codirigent_core::ImageFormat;
    /// use std::path::PathBuf;
    ///
    /// let mut preview = ClipboardPreview::new(CodirigentTheme::dark());
    /// let thumbnail = ThumbnailPreview::new(
    ///     vec![1, 2, 3],
    ///     PathBuf::from("/tmp/test.png"),
    ///     800,
    ///     600,
    ///     1000,
    ///     ImageFormat::Png,
    /// );
    /// preview.show(thumbnail);
    /// assert!(preview.is_visible());
    /// ```
    pub fn show(&mut self, preview: ThumbnailPreview) {
        self.current_preview = Some(preview);
        self.visible = true;
        // Clear cached image when preview changes
        #[cfg(feature = "gpui-full")]
        {
            self.cached_image.0 = None;
        }
    }

    /// Hide and clear the preview.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::clipboard_preview::ClipboardPreview;
    /// use codirigent_ui::smart_clipboard::ThumbnailPreview;
    /// use codirigent_ui::theme::CodirigentTheme;
    /// use codirigent_core::ImageFormat;
    /// use std::path::PathBuf;
    ///
    /// let mut preview = ClipboardPreview::new(CodirigentTheme::dark());
    /// let thumbnail = ThumbnailPreview::new(
    ///     vec![1, 2, 3],
    ///     PathBuf::from("/tmp/test.png"),
    ///     800,
    ///     600,
    ///     1000,
    ///     ImageFormat::Png,
    /// );
    /// preview.show(thumbnail);
    /// preview.hide();
    /// assert!(!preview.is_visible());
    /// assert!(preview.preview().is_none());
    /// ```
    pub fn hide(&mut self) {
        self.current_preview = None;
        self.visible = false;
        // Clear cached image when hiding
        #[cfg(feature = "gpui-full")]
        {
            self.cached_image.0 = None;
        }
    }

    /// Check if the preview is currently visible.
    ///
    /// # Returns
    ///
    /// `true` if a preview is being shown, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::clipboard_preview::ClipboardPreview;
    /// use codirigent_ui::theme::CodirigentTheme;
    ///
    /// let preview = ClipboardPreview::new(CodirigentTheme::dark());
    /// assert!(!preview.is_visible());
    /// ```
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the current preview data.
    ///
    /// # Returns
    ///
    /// A reference to the current `ThumbnailPreview` if visible, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::clipboard_preview::ClipboardPreview;
    /// use codirigent_ui::smart_clipboard::ThumbnailPreview;
    /// use codirigent_ui::theme::CodirigentTheme;
    /// use codirigent_core::ImageFormat;
    /// use std::path::PathBuf;
    ///
    /// let mut preview = ClipboardPreview::new(CodirigentTheme::dark());
    /// assert!(preview.preview().is_none());
    ///
    /// let thumbnail = ThumbnailPreview::new(
    ///     vec![1, 2, 3],
    ///     PathBuf::from("/tmp/test.png"),
    ///     800,
    ///     600,
    ///     1000,
    ///     ImageFormat::Png,
    /// );
    /// preview.show(thumbnail);
    /// assert!(preview.preview().is_some());
    /// ```
    pub fn preview(&self) -> Option<&ThumbnailPreview> {
        self.current_preview.as_ref()
    }

    /// Get a reference to the theme.
    ///
    /// # Returns
    ///
    /// A reference to the `CodirigentTheme` used for rendering.
    pub fn theme(&self) -> &CodirigentTheme {
        &self.theme
    }

    /// Update the theme.
    ///
    /// # Arguments
    ///
    /// * `theme` - The new theme to use
    pub fn set_theme(&mut self, theme: CodirigentTheme) {
        self.theme = theme;
    }

    /// Generate a thumbnail from image data, maintaining aspect ratio.
    ///
    /// Scales the image down to fit within `max_size x max_size` pixels
    /// while preserving the original aspect ratio. If the image is already
    /// smaller than the max size, it is not scaled up.
    ///
    /// # Arguments
    ///
    /// * `image` - The source image data
    /// * `max_size` - Maximum dimension (width or height) in pixels
    ///
    /// # Returns
    ///
    /// Scaled thumbnail bytes. If the image is smaller than `max_size` or
    /// if scaling fails, returns the original bytes.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::clipboard_preview::ClipboardPreview;
    /// use codirigent_core::{ImageData, ImageFormat};
    ///
    /// // Small image - returns original bytes
    /// let small_image = ImageData {
    ///     bytes: vec![1, 2, 3, 4],
    ///     width: 64,
    ///     height: 64,
    ///     format: ImageFormat::Png,
    /// };
    /// let thumbnail = ClipboardPreview::generate_thumbnail(&small_image, 128);
    /// assert_eq!(thumbnail, small_image.bytes);
    ///
    /// // Large image would be scaled down (requires valid image bytes)
    /// ```
    pub fn generate_thumbnail(image: &ImageData, max_size: u32) -> Vec<u8> {
        // If image is already smaller than max_size, return original bytes
        if image.width <= max_size && image.height <= max_size {
            return image.bytes.clone();
        }

        // Try to decode and resize the image
        match Self::resize_image_bytes(&image.bytes, max_size, image.format) {
            Ok(resized) => resized,
            Err(e) => {
                // Log warning with context and fall back to original bytes
                warn!(
                    width = image.width,
                    height = image.height,
                    format = ?image.format,
                    error = %e,
                    "Failed to resize image, returning original bytes"
                );
                image.bytes.clone()
            }
        }
    }

    /// Resize image bytes to fit within max_size while preserving aspect ratio.
    fn resize_image_bytes(
        bytes: &[u8],
        max_size: u32,
        format: CoreImageFormat,
    ) -> Result<Vec<u8>, image::ImageError> {
        // Load image from bytes
        let img = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()?
            .decode()?;

        // Resize while preserving aspect ratio (fits within max_size x max_size)
        // Using Lanczos3 for high quality thumbnails
        let thumbnail = img.resize(max_size, max_size, FilterType::Lanczos3);

        // Encode to appropriate format (preserve JPEG, convert others to PNG)
        let mut output = Vec::new();
        let output_format = match format {
            CoreImageFormat::Jpeg => image::ImageFormat::Jpeg,
            // PNG, TIFF, DIB, RGBA all get encoded as PNG for thumbnails
            CoreImageFormat::Png
            | CoreImageFormat::Tiff
            | CoreImageFormat::Dib
            | CoreImageFormat::Rgba => image::ImageFormat::Png,
        };
        thumbnail.write_to(&mut Cursor::new(&mut output), output_format)?;

        Ok(output)
    }

    /// Calculate scaled dimensions maintaining aspect ratio.
    ///
    /// # Arguments
    ///
    /// * `width` - Original width
    /// * `height` - Original height
    /// * `max_size` - Maximum dimension
    ///
    /// # Returns
    ///
    /// Tuple of (scaled_width, scaled_height)
    #[cfg(test)]
    fn calculate_scaled_dimensions(width: u32, height: u32, max_size: u32) -> (u32, u32) {
        if width == 0 || height == 0 {
            return (0, 0);
        }

        // If already smaller than max, don't scale up
        if width <= max_size && height <= max_size {
            return (width, height);
        }

        let aspect_ratio = width as f64 / height as f64;

        if width > height {
            // Landscape: constrain by width
            let new_width = max_size;
            let new_height = (max_size as f64 / aspect_ratio).round() as u32;
            (new_width, new_height.max(1))
        } else {
            // Portrait or square: constrain by height
            let new_height = max_size;
            let new_width = (max_size as f64 * aspect_ratio).round() as u32;
            (new_width.max(1), new_height)
        }
    }

    /// Create a preview from image data, path, and file size.
    ///
    /// Generates a thumbnail and creates a `ThumbnailPreview` with all
    /// the necessary metadata for display.
    ///
    /// # Arguments
    ///
    /// * `image` - The source image data
    /// * `path` - Path to the original image file
    /// * `file_size` - Size of the file in bytes
    ///
    /// # Returns
    ///
    /// A `ThumbnailPreview` ready for display.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::clipboard_preview::ClipboardPreview;
    /// use codirigent_core::{ImageData, ImageFormat};
    /// use std::path::PathBuf;
    ///
    /// let image = ImageData {
    ///     bytes: vec![0x89, 0x50, 0x4E, 0x47],
    ///     width: 1920,
    ///     height: 1080,
    ///     format: ImageFormat::Png,
    /// };
    ///
    /// let preview = ClipboardPreview::create_preview(
    ///     &image,
    ///     PathBuf::from("/tmp/screenshot.png"),
    ///     2048000,
    /// );
    ///
    /// assert_eq!(preview.original_width, 1920);
    /// assert_eq!(preview.original_height, 1080);
    /// assert_eq!(preview.file_size, 2048000);
    /// ```
    pub fn create_preview(image: &ImageData, path: PathBuf, file_size: u64) -> ThumbnailPreview {
        let thumbnail_bytes = Self::generate_thumbnail(image, MAX_THUMBNAIL_SIZE);

        // Determine the output format: JPEG stays as JPEG, everything else becomes PNG
        let output_format = match image.format {
            CoreImageFormat::Jpeg => CoreImageFormat::Jpeg,
            _ => CoreImageFormat::Png,
        };

        ThumbnailPreview::new(
            thumbnail_bytes,
            path,
            image.width,
            image.height,
            file_size,
            output_format,
        )
    }

    /// Format image dimensions as a display string.
    ///
    /// # Arguments
    ///
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    ///
    /// # Returns
    ///
    /// A formatted string like "1920x1080".
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::clipboard_preview::ClipboardPreview;
    ///
    /// let dims = ClipboardPreview::format_dimensions(1920, 1080);
    /// assert_eq!(dims, "1920x1080");
    /// ```
    pub fn format_dimensions(width: u32, height: u32) -> String {
        format!("{}x{}", width, height)
    }
}

/// GPUI Render implementation for ClipboardPreview.
///
/// Renders a thumbnail preview panel showing:
/// - 128x128 thumbnail image
/// - Original image dimensions (e.g., "1920x1080")
/// - File size in human-readable format (e.g., "1.5 MB")
/// - Path to the image (truncated if too long)
///
/// When the preview is not visible, renders an empty div.
#[cfg(feature = "gpui-full")]
impl Render for ClipboardPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Use pattern matching to safely extract preview, avoiding unwrap()
        let Some(preview) = self.current_preview.as_ref().filter(|_| self.visible) else {
            return div().into_any_element();
        };

        // Use cached GPUI Image if available, otherwise create and cache it
        let image = if let Some(cached) = self.cached_image.0.as_ref() {
            Arc::clone(cached)
        } else {
            let gpui_format = match preview.format {
                CoreImageFormat::Jpeg => ImageFormat::Jpeg,
                // PNG, TIFF, DIB, RGBA all become PNG after scaling
                _ => ImageFormat::Png,
            };
            let new_image = Arc::new(Image::from_bytes(
                gpui_format,
                preview.thumbnail_bytes.clone(),
            ));
            self.cached_image.0 = Some(Arc::clone(&new_image));
            new_image
        };

        // Convert theme colors to GPUI Hsla
        let panel_bg: gpui::Hsla = self.theme.panel_background.into();
        let border_color: gpui::Hsla = self.theme.border.into();
        let foreground_color: gpui::Hsla = self.theme.foreground.into();
        let muted_color: gpui::Hsla = self.theme.muted.into();

        div()
            .bg(panel_bg)
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                // 128x128 thumbnail image
                gpui::img(image)
                    .w(px(128.0))
                    .h(px(128.0))
                    .object_fit(ObjectFit::Contain),
            )
            .child(
                // Image dimensions
                div()
                    .text_sm()
                    .text_color(foreground_color)
                    .child(Self::format_dimensions(
                        preview.original_width,
                        preview.original_height,
                    )),
            )
            .child(
                // File size in human-readable format
                div()
                    .text_sm()
                    .text_color(muted_color)
                    .child(preview.human_readable_size()),
            )
            .child(
                // Image path (truncated)
                div()
                    .text_xs()
                    .text_color(muted_color)
                    .truncate()
                    .child(preview.image_path.display().to_string()),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::ImageFormat;

    #[test]
    fn test_clipboard_preview_new() {
        let theme = CodirigentTheme::dark();
        let preview = ClipboardPreview::new(theme.clone());

        assert!(!preview.is_visible());
        assert!(preview.preview().is_none());
        assert_eq!(preview.theme().background, theme.background);
    }

    #[test]
    fn test_clipboard_preview_show_hide() {
        let mut preview = ClipboardPreview::new(CodirigentTheme::dark());

        // Initially hidden
        assert!(!preview.is_visible());
        assert!(preview.preview().is_none());

        // Show a preview
        let thumbnail = ThumbnailPreview::new(
            vec![1, 2, 3],
            PathBuf::from("/tmp/test.png"),
            800,
            600,
            1000,
            CoreImageFormat::Png,
        );
        preview.show(thumbnail.clone());

        assert!(preview.is_visible());
        assert!(preview.preview().is_some());
        assert_eq!(preview.preview().unwrap().original_width, 800);

        // Hide the preview
        preview.hide();

        assert!(!preview.is_visible());
        assert!(preview.preview().is_none());
    }

    #[test]
    fn test_clipboard_preview_is_visible() {
        let mut preview = ClipboardPreview::new(CodirigentTheme::dark());

        // Initially not visible
        assert!(!preview.is_visible());

        // Show makes it visible
        let thumbnail =
            ThumbnailPreview::new(vec![], PathBuf::new(), 100, 100, 0, CoreImageFormat::Png);
        preview.show(thumbnail);
        assert!(preview.is_visible());

        // Hide makes it not visible
        preview.hide();
        assert!(!preview.is_visible());
    }

    #[test]
    fn test_clipboard_preview_generate_thumbnail_maintains_aspect() {
        // Landscape image (16:9)
        let landscape_image = ImageData {
            bytes: vec![1, 2, 3, 4],
            width: 1920,
            height: 1080,
            format: ImageFormat::Png,
        };
        let (w, h) = ClipboardPreview::calculate_scaled_dimensions(1920, 1080, 128);
        // Should scale to 128x72 (approximately 16:9)
        assert_eq!(w, 128);
        assert!(h > 70 && h < 75); // ~72

        // Portrait image (3:4)
        let (w, h) = ClipboardPreview::calculate_scaled_dimensions(768, 1024, 128);
        // Should scale to 96x128 (approximately 3:4)
        assert_eq!(h, 128);
        assert!(w > 94 && w < 98); // ~96

        // Square image
        let (w, h) = ClipboardPreview::calculate_scaled_dimensions(1000, 1000, 128);
        // Should scale to 128x128
        assert_eq!(w, 128);
        assert_eq!(h, 128);

        // Small image (no scaling up)
        let (w, h) = ClipboardPreview::calculate_scaled_dimensions(64, 48, 128);
        // Should stay at 64x48
        assert_eq!(w, 64);
        assert_eq!(h, 48);

        // Zero dimensions
        let (w, h) = ClipboardPreview::calculate_scaled_dimensions(0, 0, 128);
        assert_eq!(w, 0);
        assert_eq!(h, 0);

        // MVP: generate_thumbnail returns original bytes
        let thumbnail = ClipboardPreview::generate_thumbnail(&landscape_image, 128);
        assert_eq!(thumbnail, landscape_image.bytes);
    }

    #[test]
    fn test_clipboard_preview_create_preview() {
        let image = ImageData {
            bytes: vec![0x89, 0x50, 0x4E, 0x47],
            width: 1920,
            height: 1080,
            format: ImageFormat::Png,
        };
        let path = PathBuf::from("/tmp/screenshot.png");
        let file_size = 2048000u64;

        let preview = ClipboardPreview::create_preview(&image, path.clone(), file_size);

        assert_eq!(preview.original_width, 1920);
        assert_eq!(preview.original_height, 1080);
        assert_eq!(preview.file_size, 2048000);
        assert_eq!(preview.image_path, path);
        // MVP: thumbnail bytes are the original bytes
        assert_eq!(preview.thumbnail_bytes, image.bytes);
    }

    #[test]
    fn test_clipboard_preview_format_dimensions() {
        assert_eq!(ClipboardPreview::format_dimensions(1920, 1080), "1920x1080");
        assert_eq!(ClipboardPreview::format_dimensions(800, 600), "800x600");
        assert_eq!(ClipboardPreview::format_dimensions(0, 0), "0x0");
        assert_eq!(ClipboardPreview::format_dimensions(1, 1), "1x1");
    }

    #[test]
    fn test_clipboard_preview_theme_methods() {
        let dark_theme = CodirigentTheme::dark();
        let light_theme = CodirigentTheme::light();

        let mut preview = ClipboardPreview::new(dark_theme.clone());
        assert_eq!(preview.theme().background, dark_theme.background);

        preview.set_theme(light_theme.clone());
        assert_eq!(preview.theme().background, light_theme.background);
    }

    #[test]
    fn test_clipboard_preview_clone() {
        let mut original = ClipboardPreview::new(CodirigentTheme::dark());
        let thumbnail = ThumbnailPreview::new(
            vec![1, 2, 3],
            PathBuf::from("/tmp/test.png"),
            800,
            600,
            1000,
            CoreImageFormat::Png,
        );
        original.show(thumbnail);

        let cloned = original.clone();
        assert_eq!(cloned.is_visible(), original.is_visible());
        assert!(cloned.preview().is_some());
        assert_eq!(
            cloned.preview().unwrap().original_width,
            original.preview().unwrap().original_width
        );
    }

    #[test]
    fn test_clipboard_preview_debug() {
        let preview = ClipboardPreview::new(CodirigentTheme::dark());
        let debug_str = format!("{:?}", preview);
        assert!(debug_str.contains("ClipboardPreview"));
        assert!(debug_str.contains("visible: false"));
    }

    #[test]
    fn test_max_thumbnail_size_constant() {
        assert_eq!(MAX_THUMBNAIL_SIZE, 128);
    }

    #[test]
    fn test_scaled_dimensions_edge_cases() {
        // Very wide image
        let (w, h) = ClipboardPreview::calculate_scaled_dimensions(10000, 100, 128);
        assert_eq!(w, 128);
        assert!(h >= 1); // Should be at least 1

        // Very tall image
        let (w, h) = ClipboardPreview::calculate_scaled_dimensions(100, 10000, 128);
        assert_eq!(h, 128);
        assert!(w >= 1); // Should be at least 1
    }
}
