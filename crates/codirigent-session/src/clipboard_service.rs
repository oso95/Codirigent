//! Clipboard service for Smart Clipboard functionality.
//!
//! This module provides a service for handling clipboard operations,
//! including saving images, formatting content for different CLI types,
//! and managing temporary files.
//!
//! # Example
//!
//! ```no_run
//! use codirigent_session::clipboard_service::{ClipboardService, DefaultClipboardService};
//! use codirigent_core::{ClipboardContent, CliType, ImageData, ImageFormat, SessionId};
//! use std::path::Path;
//! use std::time::Duration;
//!
//! let service = DefaultClipboardService::new(Path::new("/project/.codirigent"));
//!
//! // Format text for a CLI
//! let content = ClipboardContent::Text("Hello, world!".to_string());
//! let formatted = service.format_for_cli(&content, CliType::ClaudeCode).unwrap();
//! assert_eq!(formatted, "Hello, world!");
//!
//! // Get CLI type for a session
//! let cli_type = service.get_session_cli_type(SessionId(1));
//! assert_eq!(cli_type, CliType::ClaudeCode); // default
//! ```

use anyhow::{Context, Result};
use chrono::Local;
use codirigent_core::{CliType, ClipboardContent, ImageData, ImageFormat, SessionId};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Service for smart clipboard operations.
///
/// Provides methods for saving clipboard images, formatting content for
/// different CLI types, and managing temporary files used during clipboard
/// operations.
pub trait ClipboardService: Send + Sync {
    /// Save clipboard image to the temp directory.
    ///
    /// Saves the image bytes to a file in the temp directory with a
    /// timestamp-based filename.
    ///
    /// # Arguments
    ///
    /// * `image` - The image data to save
    ///
    /// # Returns
    ///
    /// The path to the saved image file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    #[must_use = "the saved image path should be used"]
    fn save_image(&self, image: &ImageData) -> Result<PathBuf>;

    /// Format content for a specific CLI type.
    ///
    /// Transforms clipboard content into a string suitable for pasting
    /// into the specified CLI. Images are saved to temp files and the
    /// path is formatted according to CLI conventions.
    ///
    /// # Arguments
    ///
    /// * `content` - The clipboard content to format
    /// * `cli_type` - The target CLI type
    ///
    /// # Returns
    ///
    /// A formatted string suitable for the CLI.
    ///
    /// # Errors
    ///
    /// Returns an error if image saving fails.
    #[must_use = "the formatted content should be used"]
    fn format_for_cli(&self, content: &ClipboardContent, cli_type: CliType) -> Result<String>;

    /// Get the CLI type for a session.
    ///
    /// Returns the stored CLI type for the session, or the default
    /// (ClaudeCode) if not explicitly set.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to get the CLI type for
    ///
    /// # Returns
    ///
    /// The CLI type for the session.
    #[must_use = "the CLI type should be used"]
    fn get_session_cli_type(&self, session_id: SessionId) -> CliType;

    /// Set the CLI type for a session (manual override).
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to set the CLI type for
    /// * `cli_type` - The CLI type to set
    fn set_session_cli_type(&mut self, session_id: SessionId, cli_type: CliType);

    /// Get the temp directory path.
    ///
    /// Returns the path to the temp directory where clipboard files
    /// are stored. Creates the directory if it doesn't exist.
    ///
    /// # Returns
    ///
    /// The path to the temp directory.
    #[must_use = "the temp directory path should be used"]
    fn temp_dir(&self) -> &Path;

    /// Clean up old temp files (older than specified duration).
    ///
    /// Removes files in the temp directory that are older than the
    /// specified maximum age.
    ///
    /// # Arguments
    ///
    /// * `max_age` - Maximum age of files to keep
    ///
    /// # Returns
    ///
    /// The number of files removed.
    ///
    /// # Errors
    ///
    /// Returns an error if the temp directory cannot be read.
    #[must_use = "the cleanup count should be checked"]
    fn cleanup_temp_files(&self, max_age: Duration) -> Result<usize>;
}

/// Default implementation of the clipboard service.
///
/// Provides file-based clipboard operations with timestamp-based filenames
/// and per-session CLI type tracking.
#[derive(Debug)]
pub struct DefaultClipboardService {
    /// Cached temp directory path (.codirigent/temp)
    temp_dir: PathBuf,
    /// CLI type per session
    session_cli_types: HashMap<SessionId, CliType>,
}

impl DefaultClipboardService {
    /// Create a new clipboard service.
    ///
    /// # Arguments
    ///
    /// * `base_path` - Path to the .codirigent directory
    ///
    /// # Returns
    ///
    /// A new `DefaultClipboardService` instance.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_session::clipboard_service::DefaultClipboardService;
    /// use std::path::Path;
    ///
    /// let service = DefaultClipboardService::new(Path::new("/project/.codirigent"));
    /// ```
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        let temp_dir = base_path.as_ref().join("temp");
        Self {
            temp_dir,
            session_cli_types: HashMap::new(),
        }
    }

    /// Generate a unique filename for a clipboard image.
    ///
    /// Creates a filename based on the current timestamp with format:
    /// `clipboard_YYYYMMDD_HHMMSS.ext`
    ///
    /// If a file with that name already exists, appends a counter:
    /// `clipboard_YYYYMMDD_HHMMSS_1.ext`
    fn generate_filename(&self, extension: &str) -> PathBuf {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let base_name = format!("clipboard_{}.{}", timestamp, extension);
        let mut path = self.temp_dir.join(&base_name);

        // If file exists, add a counter suffix
        let mut counter = 1;
        while path.exists() {
            let name_with_counter =
                format!("clipboard_{}_{}.{}", timestamp, counter, extension);
            path = self.temp_dir.join(name_with_counter);
            counter += 1;
        }

        path
    }

    /// Ensure the temp directory exists.
    fn ensure_temp_dir(&self) -> Result<PathBuf> {
        if !self.temp_dir.exists() {
            fs::create_dir_all(&self.temp_dir)?;
        }
        Ok(self.temp_dir.clone())
    }
}

/// Convert raw Windows DIB (BITMAPINFOHEADER + pixel data) to RGBA pixel buffer.
///
/// DIB from CF_DIB clipboard format has this layout:
/// - BITMAPINFOHEADER (40+ bytes): header with dimensions, bit depth, compression
/// - Optional color table (for <= 8 bpp, or BI_BITFIELDS masks)
/// - Pixel data (bottom-up by default, or top-down if height is negative)
///
/// This function handles 32-bit (BGRA/BGRX) and 24-bit (BGR) bitmaps,
/// which are the most common formats from Windows screenshots.
fn dib_to_rgba(dib: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    if dib.len() < 40 {
        anyhow::bail!("DIB data too small: {} bytes", dib.len());
    }

    // Parse BITMAPINFOHEADER fields
    let header_size = u32::from_le_bytes([dib[0], dib[1], dib[2], dib[3]]);
    let bi_width = i32::from_le_bytes([dib[4], dib[5], dib[6], dib[7]]);
    let bi_height = i32::from_le_bytes([dib[8], dib[9], dib[10], dib[11]]);
    let bi_bit_count = u16::from_le_bytes([dib[14], dib[15]]);
    let bi_compression = u32::from_le_bytes([dib[16], dib[17], dib[18], dib[19]]);

    let w = bi_width.unsigned_abs() as usize;
    let h = bi_height.unsigned_abs() as usize;
    let bottom_up = bi_height > 0; // Positive height = bottom-up rows

    // Use the passed-in width/height as fallback (they should match)
    let w = if w > 0 { w } else { width as usize };
    let h = if h > 0 { h } else { height as usize };

    // Calculate pixel data offset: header + optional color masks/table
    let pixel_offset = match (bi_bit_count, bi_compression) {
        (32, 3) => header_size as usize + 12, // BI_BITFIELDS: 3 x u32 color masks
        (16, 3) => header_size as usize + 12, // BI_BITFIELDS for 16bpp
        _ => header_size as usize,            // No extra data for 24/32 bpp BI_RGB
    };

    if pixel_offset >= dib.len() {
        anyhow::bail!(
            "DIB pixel offset {} exceeds data length {}",
            pixel_offset,
            dib.len()
        );
    }

    let pixel_data = &dib[pixel_offset..];
    let bytes_per_pixel = (bi_bit_count as usize) / 8;
    // BMP rows are padded to 4-byte boundaries
    let row_stride = ((w * bytes_per_pixel + 3) / 4) * 4;

    let mut rgba = vec![255u8; w * h * 4]; // Pre-fill alpha to 255

    for y in 0..h {
        let src_y = if bottom_up { h - 1 - y } else { y };
        let row_start = src_y * row_stride;

        if row_start + w * bytes_per_pixel > pixel_data.len() {
            break; // Truncated data — render what we can
        }

        for x in 0..w {
            let src_offset = row_start + x * bytes_per_pixel;
            let dst_offset = (y * w + x) * 4;

            match bi_bit_count {
                32 => {
                    // BGRA or BGRX → RGBA
                    rgba[dst_offset] = pixel_data[src_offset + 2]; // R
                    rgba[dst_offset + 1] = pixel_data[src_offset + 1]; // G
                    rgba[dst_offset + 2] = pixel_data[src_offset]; // B
                    rgba[dst_offset + 3] = pixel_data[src_offset + 3]; // A (or X=0)
                    // Many screenshots have A=0 (BGRX). Force opaque.
                    if rgba[dst_offset + 3] == 0 {
                        rgba[dst_offset + 3] = 255;
                    }
                }
                24 => {
                    // BGR → RGBA
                    rgba[dst_offset] = pixel_data[src_offset + 2]; // R
                    rgba[dst_offset + 1] = pixel_data[src_offset + 1]; // G
                    rgba[dst_offset + 2] = pixel_data[src_offset]; // B
                    rgba[dst_offset + 3] = 255; // A
                }
                _ => {
                    anyhow::bail!("Unsupported DIB bit depth: {}", bi_bit_count);
                }
            }
        }
    }

    Ok(rgba)
}

impl ClipboardService for DefaultClipboardService {
    fn save_image(&self, image: &ImageData) -> Result<PathBuf> {
        // Ensure temp directory exists
        self.ensure_temp_dir()?;

        // Always save as PNG for maximum compatibility with CLI tools.
        // Windows clipboard provides DIB format which is large and poorly supported.
        let path = self.generate_filename("png");

        match image.format {
            ImageFormat::Png => {
                // Already PNG — write directly
                fs::write(&path, &image.bytes)?;
            }
            ImageFormat::Dib => {
                // Windows clipboard may return either:
                // a) Full BMP file (starts with "BM" magic) — from clipboard-win
                // b) Raw DIB data (BITMAPINFOHEADER + pixels, no file header)
                let is_full_bmp = image.bytes.len() >= 2
                    && image.bytes[0] == b'B'
                    && image.bytes[1] == b'M';

                if is_full_bmp {
                    let decoded = image::load_from_memory_with_format(
                        &image.bytes,
                        image::ImageFormat::Bmp,
                    )
                    .context("Failed to decode BMP clipboard image")?;
                    decoded
                        .save_with_format(&path, image::ImageFormat::Png)
                        .context("Failed to encode BMP as PNG")?;
                } else {
                    let rgba = dib_to_rgba(&image.bytes, image.width, image.height)
                        .context("Failed to convert raw DIB to RGBA")?;
                    let rgba_image =
                        image::RgbaImage::from_raw(image.width, image.height, rgba)
                            .context("Failed to create RGBA image from DIB")?;
                    rgba_image
                        .save_with_format(&path, image::ImageFormat::Png)
                        .context("Failed to encode DIB as PNG")?;
                }
            }
            ImageFormat::Tiff | ImageFormat::Jpeg => {
                // Decode from source format and re-encode as PNG
                let decoded = image::load_from_memory(&image.bytes)
                    .context("Failed to decode clipboard image")?;
                decoded
                    .save_with_format(&path, image::ImageFormat::Png)
                    .context("Failed to encode image as PNG")?;
            }
            ImageFormat::Rgba => {
                // Raw RGBA pixel data — construct image from dimensions
                let rgba_image = image::RgbaImage::from_raw(
                    image.width,
                    image.height,
                    image.bytes.clone(),
                )
                .context("Invalid RGBA dimensions for clipboard image")?;
                rgba_image
                    .save_with_format(&path, image::ImageFormat::Png)
                    .context("Failed to encode RGBA image as PNG")?;
            }
        }

        Ok(path)
    }

    fn format_for_cli(&self, content: &ClipboardContent, cli_type: CliType) -> Result<String> {
        match content {
            ClipboardContent::Text(text) => Ok(text.clone()),
            ClipboardContent::Image(image) => {
                let path = self.save_image(image)?;
                Ok(cli_type.format_image_input(&path))
            }
            ClipboardContent::Files(files) => {
                let formatted: Vec<String> = files
                    .iter()
                    .map(|p| cli_type.format_image_input(p))
                    .collect();
                Ok(formatted.join(" "))
            }
            ClipboardContent::Empty => Ok(String::new()),
        }
    }

    fn get_session_cli_type(&self, session_id: SessionId) -> CliType {
        self.session_cli_types
            .get(&session_id)
            .copied()
            .unwrap_or_default()
    }

    fn set_session_cli_type(&mut self, session_id: SessionId, cli_type: CliType) {
        self.session_cli_types.insert(session_id, cli_type);
    }

    fn temp_dir(&self) -> &Path {
        // Create the directory if it doesn't exist (ignore errors here, they'll surface on actual use)
        let _ = fs::create_dir_all(&self.temp_dir);
        &self.temp_dir
    }

    fn cleanup_temp_files(&self, max_age: Duration) -> Result<usize> {
        if !self.temp_dir.exists() {
            return Ok(0);
        }

        let now = std::time::SystemTime::now();
        let mut removed_count = 0;

        for entry in fs::read_dir(&self.temp_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Get file modification time
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > max_age {
                            if fs::remove_file(&path).is_ok() {
                                removed_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::ImageFormat;
    use std::thread;
    use std::time::Duration as StdDuration;
    use tempfile::TempDir;

    fn create_test_service() -> (DefaultClipboardService, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let service = DefaultClipboardService::new(temp_dir.path());
        (service, temp_dir)
    }

    fn create_test_image() -> ImageData {
        ImageData {
            bytes: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            width: 100,
            height: 100,
            format: ImageFormat::Png,
        }
    }

    #[test]
    fn test_new_creates_service() {
        let temp_dir = TempDir::new().unwrap();
        let service = DefaultClipboardService::new(temp_dir.path());
        assert_eq!(service.temp_dir, temp_dir.path().join("temp"));
        assert!(service.session_cli_types.is_empty());
    }

    #[test]
    fn test_save_image_creates_file() {
        let (service, _temp_dir) = create_test_service();
        let image = create_test_image();

        let path = service.save_image(&image).unwrap();

        assert!(path.exists());
        assert!(path.is_file());

        let content = fs::read(&path).unwrap();
        assert_eq!(content, image.bytes);
    }

    #[test]
    fn test_save_image_uses_timestamp_format() {
        let (service, _temp_dir) = create_test_service();
        let image = create_test_image();

        let path = service.save_image(&image).unwrap();

        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(filename.starts_with("clipboard_"));
        assert!(filename.ends_with(".png"));
        // Check format: clipboard_YYYYMMDD_HHMMSS.png
        assert!(filename.len() >= "clipboard_20240101_120000.png".len());
    }

    #[test]
    fn test_save_image_creates_temp_dir() {
        let temp_dir = TempDir::new().unwrap();
        let service = DefaultClipboardService::new(temp_dir.path());
        let image = create_test_image();

        // Temp dir shouldn't exist yet
        let temp_path = temp_dir.path().join("temp");
        assert!(!temp_path.exists());

        // Save image should create it
        let _path = service.save_image(&image).unwrap();

        assert!(temp_path.exists());
        assert!(temp_path.is_dir());
    }

    #[test]
    fn test_format_for_cli_text_passthrough() {
        let (service, _temp_dir) = create_test_service();
        let content = ClipboardContent::Text("Hello, world!".to_string());

        let formatted = service
            .format_for_cli(&content, CliType::ClaudeCode)
            .unwrap();
        assert_eq!(formatted, "Hello, world!");

        let formatted = service
            .format_for_cli(&content, CliType::GeminiCli)
            .unwrap();
        assert_eq!(formatted, "Hello, world!");

        let formatted = service
            .format_for_cli(&content, CliType::CodexCli)
            .unwrap();
        assert_eq!(formatted, "Hello, world!");
    }

    #[test]
    fn test_format_for_cli_image_claude() {
        let (service, _temp_dir) = create_test_service();
        let image = create_test_image();
        let content = ClipboardContent::Image(image);

        let formatted = service
            .format_for_cli(&content, CliType::ClaudeCode)
            .unwrap();

        // Should be a plain path (no prefix for Claude)
        assert!(!formatted.starts_with('@'));
        assert!(formatted.contains("clipboard_"));
        assert!(formatted.ends_with(".png"));
    }

    #[test]
    fn test_format_for_cli_image_gemini() {
        let (service, _temp_dir) = create_test_service();
        let image = create_test_image();
        let content = ClipboardContent::Image(image);

        let formatted = service
            .format_for_cli(&content, CliType::GeminiCli)
            .unwrap();

        // Should have @ prefix for Gemini
        assert!(formatted.starts_with('@'));
        assert!(formatted.contains("clipboard_"));
        assert!(formatted.ends_with(".png"));
    }

    #[test]
    fn test_format_for_cli_files() {
        let (service, _temp_dir) = create_test_service();
        let files = vec![
            PathBuf::from("/tmp/file1.txt"),
            PathBuf::from("/tmp/file2.txt"),
        ];
        let content = ClipboardContent::Files(files);

        // Claude: plain paths
        let formatted = service
            .format_for_cli(&content, CliType::ClaudeCode)
            .unwrap();
        assert_eq!(formatted, "/tmp/file1.txt /tmp/file2.txt");

        // Gemini: @ prefix
        let formatted = service
            .format_for_cli(&content, CliType::GeminiCli)
            .unwrap();
        assert_eq!(formatted, "@/tmp/file1.txt @/tmp/file2.txt");
    }

    #[test]
    fn test_format_for_cli_empty() {
        let (service, _temp_dir) = create_test_service();
        let content = ClipboardContent::Empty;

        let formatted = service
            .format_for_cli(&content, CliType::ClaudeCode)
            .unwrap();
        assert_eq!(formatted, "");

        let formatted = service
            .format_for_cli(&content, CliType::GeminiCli)
            .unwrap();
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_session_cli_type_default_is_claude() {
        let (service, _temp_dir) = create_test_service();

        let cli_type = service.get_session_cli_type(SessionId(1));
        assert_eq!(cli_type, CliType::ClaudeCode);

        let cli_type = service.get_session_cli_type(SessionId(999));
        assert_eq!(cli_type, CliType::ClaudeCode);
    }

    #[test]
    fn test_session_cli_type_set_and_get() {
        let (mut service, _temp_dir) = create_test_service();

        // Set to Gemini
        service.set_session_cli_type(SessionId(1), CliType::GeminiCli);
        assert_eq!(
            service.get_session_cli_type(SessionId(1)),
            CliType::GeminiCli
        );

        // Set to Codex
        service.set_session_cli_type(SessionId(2), CliType::CodexCli);
        assert_eq!(
            service.get_session_cli_type(SessionId(2)),
            CliType::CodexCli
        );

        // Session 1 should still be Gemini
        assert_eq!(
            service.get_session_cli_type(SessionId(1)),
            CliType::GeminiCli
        );

        // Override session 1 to Generic
        service.set_session_cli_type(SessionId(1), CliType::GenericShell);
        assert_eq!(
            service.get_session_cli_type(SessionId(1)),
            CliType::GenericShell
        );
    }

    #[test]
    fn test_temp_dir_returns_correct_path() {
        let (service, temp_dir) = create_test_service();

        let returned_path = service.temp_dir();

        // Should end with "temp"
        assert!(returned_path.ends_with("temp"));
        // Should be under the base path
        assert!(returned_path.starts_with(temp_dir.path()));
    }

    #[test]
    fn test_cleanup_removes_old_files() {
        let (service, temp_dir) = create_test_service();

        // Create temp directory
        let temp_path = temp_dir.path().join("temp");
        fs::create_dir_all(&temp_path).unwrap();

        // Create some test files
        let old_file = temp_path.join("old_file.txt");
        let new_file = temp_path.join("new_file.txt");

        fs::write(&old_file, "old content").unwrap();
        fs::write(&new_file, "new content").unwrap();

        // Set old file's modification time to the past by sleeping and checking
        // We need to use a different approach since we can't easily set mtime.
        // Instead, we'll create a file, wait briefly, then check cleanup with a very short duration.

        // Small sleep to ensure time passes
        thread::sleep(StdDuration::from_millis(100));

        // Create another file after the sleep
        let newer_file = temp_path.join("newer_file.txt");
        fs::write(&newer_file, "newer content").unwrap();

        // Cleanup files older than 50ms (old_file and new_file should be removed)
        let removed = service
            .cleanup_temp_files(StdDuration::from_millis(50))
            .unwrap();

        // At least the first two files should be removed
        assert!(removed >= 2);
        assert!(!old_file.exists());
        assert!(!new_file.exists());
    }

    #[test]
    fn test_cleanup_keeps_new_files() {
        let (service, temp_dir) = create_test_service();

        // Create temp directory and a new file
        let temp_path = temp_dir.path().join("temp");
        fs::create_dir_all(&temp_path).unwrap();

        let new_file = temp_path.join("new_file.txt");
        fs::write(&new_file, "new content").unwrap();

        // Cleanup files older than 1 hour (new file should be kept)
        let removed = service
            .cleanup_temp_files(StdDuration::from_secs(3600))
            .unwrap();

        assert_eq!(removed, 0);
        assert!(new_file.exists());
    }

    #[test]
    fn test_cleanup_handles_missing_temp_dir() {
        let (service, _temp_dir) = create_test_service();

        // Don't create temp dir - cleanup should handle this gracefully
        let removed = service
            .cleanup_temp_files(StdDuration::from_secs(0))
            .unwrap();

        assert_eq!(removed, 0);
    }

    #[test]
    fn test_save_image_always_outputs_png() {
        let (service, _temp_dir) = create_test_service();
        // Use a valid PNG for this test (all formats save as .png)
        let image = create_test_image(); // PNG format

        let path = service.save_image(&image).unwrap();

        assert!(path.exists());
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(
            filename.ends_with(".png"),
            "All clipboard images should save as PNG, got: {}",
            filename
        );
    }

    #[test]
    fn test_save_image_unique_names_for_same_timestamp() {
        let (service, _temp_dir) = create_test_service();
        let image = create_test_image();

        // Save multiple images quickly - they should get unique names
        let path1 = service.save_image(&image).unwrap();
        let path2 = service.save_image(&image).unwrap();
        let path3 = service.save_image(&image).unwrap();

        // All paths should be different
        assert_ne!(path1, path2);
        assert_ne!(path2, path3);
        assert_ne!(path1, path3);

        // All files should exist
        assert!(path1.exists());
        assert!(path2.exists());
        assert!(path3.exists());
    }

    #[test]
    fn test_format_for_cli_files_single_file() {
        let (service, _temp_dir) = create_test_service();
        let files = vec![PathBuf::from("/tmp/single.txt")];
        let content = ClipboardContent::Files(files);

        let formatted = service
            .format_for_cli(&content, CliType::ClaudeCode)
            .unwrap();
        assert_eq!(formatted, "/tmp/single.txt");
    }

    #[test]
    fn test_format_for_cli_files_empty_vec() {
        let (service, _temp_dir) = create_test_service();
        let content = ClipboardContent::Files(vec![]);

        let formatted = service
            .format_for_cli(&content, CliType::ClaudeCode)
            .unwrap();
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_format_for_cli_text_empty_string() {
        let (service, _temp_dir) = create_test_service();
        let content = ClipboardContent::Text(String::new());

        let formatted = service
            .format_for_cli(&content, CliType::ClaudeCode)
            .unwrap();
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_format_for_cli_text_with_special_chars() {
        let (service, _temp_dir) = create_test_service();
        let content = ClipboardContent::Text("Hello\n\tWorld! @#$%".to_string());

        let formatted = service
            .format_for_cli(&content, CliType::ClaudeCode)
            .unwrap();
        assert_eq!(formatted, "Hello\n\tWorld! @#$%");
    }

    #[test]
    fn test_service_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DefaultClipboardService>();
    }

    #[test]
    fn test_format_for_cli_codex() {
        let (service, _temp_dir) = create_test_service();
        let image = create_test_image();
        let content = ClipboardContent::Image(image);

        let formatted = service
            .format_for_cli(&content, CliType::CodexCli)
            .unwrap();

        // Codex should be like Claude - no prefix
        assert!(!formatted.starts_with('@'));
        assert!(formatted.contains("clipboard_"));
        assert!(formatted.ends_with(".png"));
    }

    #[test]
    fn test_format_for_cli_generic_shell() {
        let (service, _temp_dir) = create_test_service();
        let image = create_test_image();
        let content = ClipboardContent::Image(image);

        let formatted = service
            .format_for_cli(&content, CliType::GenericShell)
            .unwrap();

        // Generic shell should be like Claude - no prefix
        assert!(!formatted.starts_with('@'));
        assert!(formatted.contains("clipboard_"));
        assert!(formatted.ends_with(".png"));
    }

    #[test]
    fn test_cleanup_ignores_directories() {
        let (service, temp_dir) = create_test_service();

        // Create temp directory with a subdirectory
        let temp_path = temp_dir.path().join("temp");
        fs::create_dir_all(&temp_path).unwrap();

        let subdir = temp_path.join("subdir");
        fs::create_dir_all(&subdir).unwrap();

        // Also create a file
        let file = temp_path.join("test.txt");
        fs::write(&file, "test").unwrap();

        // Small sleep
        thread::sleep(StdDuration::from_millis(100));

        // Cleanup should not remove directories
        let removed = service
            .cleanup_temp_files(StdDuration::from_millis(50))
            .unwrap();

        assert_eq!(removed, 1); // Only the file
        assert!(subdir.exists()); // Directory still exists
    }
}
