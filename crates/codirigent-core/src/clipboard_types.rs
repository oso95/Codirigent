//! Clipboard types for Smart Clipboard functionality.
//!
//! This module provides types for handling clipboard content across
//! different CLI tools (Claude Code, Gemini CLI, Codex CLI) with
//! proper formatting for each CLI's input requirements.
//!
//! ## CLI Type Detection
//!
//! The [`CliType`] enum represents different AI CLI tools and provides
//! methods to detect the CLI from process information and format input
//! appropriately for each CLI.
//!
//! ```
//! use codirigent_core::clipboard_types::CliType;
//! use std::path::Path;
//!
//! // Detect CLI from process name
//! let cli = CliType::detect("claude", None);
//! assert_eq!(cli, CliType::ClaudeCode);
//!
//! // Format image input for the CLI
//! let formatted = cli.format_image_input(Path::new("/tmp/image.png"));
//! assert_eq!(formatted, "/tmp/image.png");
//!
//! // Gemini uses @ prefix for files
//! let gemini = CliType::GeminiCli;
//! let formatted = gemini.format_image_input(Path::new("/tmp/image.png"));
//! assert_eq!(formatted, "@/tmp/image.png");
//! ```
//!
//! ## Clipboard Content
//!
//! The [`ClipboardContent`] enum represents different types of content
//! that can be stored in the clipboard:
//!
//! - Text content
//! - Image data (with dimensions and format)
//! - File paths
//! - Empty clipboard
//!
//! ```
//! use codirigent_core::clipboard_types::{ClipboardContent, ImageData, ImageFormat};
//! use codirigent_core::ClipboardContentType;
//! use std::path::PathBuf;
//!
//! // Text content
//! let text = ClipboardContent::Text("Hello, world!".to_string());
//! assert_eq!(text.content_type(), ClipboardContentType::Text);
//!
//! // Image content
//! let image = ClipboardContent::Image(ImageData {
//!     bytes: vec![0x89, 0x50, 0x4E, 0x47], // PNG magic bytes
//!     width: 100,
//!     height: 100,
//!     format: ImageFormat::Png,
//! });
//!
//! // File paths
//! let files = ClipboardContent::Files(vec![
//!     PathBuf::from("/tmp/file1.txt"),
//!     PathBuf::from("/tmp/file2.txt"),
//! ]);
//! ```

use crate::events::ClipboardContentType;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;

/// Type of CLI tool being used in a session.
///
/// Different CLI tools have different requirements for input formatting,
/// particularly for images and file references. This enum allows the
/// clipboard system to format content appropriately for each CLI.
///
/// # Example
///
/// ```
/// use codirigent_core::clipboard_types::CliType;
///
/// let cli = CliType::default();
/// assert_eq!(cli, CliType::GenericShell);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CliType {
    /// Claude Code CLI.
    ClaudeCode,
    /// Gemini CLI - uses @ prefix for file references.
    GeminiCli,
    /// Codex CLI.
    CodexCli,
    /// Generic shell or unknown CLI (default for new sessions).
    #[default]
    GenericShell,
}

impl CliType {
    /// Format an image file path for input to this CLI.
    ///
    /// Different CLIs have different conventions for referencing files:
    /// - Claude Code: Uses plain file paths
    /// - Gemini CLI: Uses `@` prefix for file references
    /// - Codex CLI: Uses plain file paths
    /// - Generic Shell: Uses plain file paths
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the image file
    ///
    /// # Returns
    ///
    /// A formatted string suitable for pasting into the CLI.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::clipboard_types::CliType;
    /// use std::path::Path;
    ///
    /// let gemini = CliType::GeminiCli;
    /// let formatted = gemini.format_image_input(Path::new("/tmp/image.png"));
    /// assert_eq!(formatted, "@/tmp/image.png");
    ///
    /// let claude = CliType::ClaudeCode;
    /// let formatted = claude.format_image_input(Path::new("/tmp/image.png"));
    /// assert_eq!(formatted, "/tmp/image.png");
    /// ```
    pub fn format_image_input(&self, path: &Path) -> String {
        let path_str = path.display().to_string();
        match self {
            CliType::GeminiCli => format!("@{}", path_str),
            CliType::ClaudeCode | CliType::CodexCli | CliType::GenericShell => path_str,
        }
    }

    /// Detect the CLI type from process information.
    ///
    /// Attempts to determine which CLI is running based on the process name
    /// and optionally the command line arguments. Detection is case-insensitive
    /// and looks for common patterns in process names.
    ///
    /// # Arguments
    ///
    /// * `process_name` - The name of the process (e.g., "claude", "gemini")
    /// * `command_line` - Optional command line string for additional context
    ///
    /// # Returns
    ///
    /// The detected [`CliType`], or [`CliType::GenericShell`] if unknown.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::clipboard_types::CliType;
    ///
    /// // Detect from process name
    /// assert_eq!(CliType::detect("claude", None), CliType::ClaudeCode);
    /// assert_eq!(CliType::detect("gemini", None), CliType::GeminiCli);
    /// assert_eq!(CliType::detect("codex", None), CliType::CodexCli);
    ///
    /// // Detect from command line
    /// assert_eq!(
    ///     CliType::detect("node", Some("node /usr/bin/claude")),
    ///     CliType::ClaudeCode
    /// );
    ///
    /// // Unknown falls back to GenericShell
    /// assert_eq!(CliType::detect("bash", None), CliType::GenericShell);
    /// ```
    pub fn detect(process_name: &str, command_line: Option<&str>) -> Self {
        let process_lower = process_name.to_lowercase();

        // Check process name first
        if process_lower.contains("claude") {
            return CliType::ClaudeCode;
        }
        if process_lower.contains("gemini") {
            return CliType::GeminiCli;
        }
        if process_lower.contains("codex") {
            return CliType::CodexCli;
        }

        // Check command line if available
        if let Some(cmd) = command_line {
            let cmd_lower = cmd.to_lowercase();
            if cmd_lower.contains("claude") {
                return CliType::ClaudeCode;
            }
            if cmd_lower.contains("gemini") {
                return CliType::GeminiCli;
            }
            if cmd_lower.contains("codex") {
                return CliType::CodexCli;
            }
        }

        CliType::GenericShell
    }
}

/// Image format for clipboard image data.
///
/// Supported formats for images stored in the clipboard.
///
/// # Example
///
/// ```
/// use codirigent_core::clipboard_types::ImageFormat;
///
/// let png = ImageFormat::Png;
/// assert_eq!(png.extension(), "png");
///
/// let jpeg = ImageFormat::Jpeg;
/// assert_eq!(jpeg.extension(), "jpg");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    /// PNG format (lossless, preferred).
    Png,
    /// JPEG format (lossy).
    Jpeg,
    /// TIFF format (from macOS clipboard).
    Tiff,
    /// Windows DIB (Device Independent Bitmap) format.
    Dib,
    /// Raw RGBA pixel data (from arboard/Linux clipboard).
    Rgba,
}

impl ImageFormat {
    /// Get the file extension for this image format.
    ///
    /// # Returns
    ///
    /// The file extension string (without leading dot).
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::clipboard_types::ImageFormat;
    ///
    /// assert_eq!(ImageFormat::Png.extension(), "png");
    /// assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
    /// assert_eq!(ImageFormat::Tiff.extension(), "tiff");
    /// assert_eq!(ImageFormat::Dib.extension(), "bmp");
    /// assert_eq!(ImageFormat::Rgba.extension(), "raw");
    /// ```
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Tiff => "tiff",
            ImageFormat::Dib => "bmp",
            ImageFormat::Rgba => "raw",
        }
    }
}

/// Image data from the clipboard.
///
/// Contains the raw image bytes along with metadata about the image
/// dimensions and format.
///
/// # Example
///
/// ```
/// use codirigent_core::clipboard_types::{ImageData, ImageFormat};
///
/// let data = ImageData {
///     bytes: vec![0x89, 0x50, 0x4E, 0x47], // PNG magic bytes
///     width: 1920,
///     height: 1080,
///     format: ImageFormat::Png,
/// };
///
/// assert_eq!(data.width, 1920);
/// assert_eq!(data.height, 1080);
/// assert_eq!(data.format, ImageFormat::Png);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageData {
    /// Raw image bytes.
    pub bytes: Vec<u8>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Image format.
    pub format: ImageFormat,
}

/// Content stored in the clipboard.
///
/// Represents the different types of content that can be stored in
/// or retrieved from the system clipboard.
///
/// # Example
///
/// ```
/// use codirigent_core::clipboard_types::ClipboardContent;
/// use codirigent_core::ClipboardContentType;
/// use std::path::PathBuf;
///
/// // Text content
/// let text = ClipboardContent::Text("Hello, world!".to_string());
/// assert_eq!(text.content_type(), ClipboardContentType::Text);
///
/// // File paths
/// let files = ClipboardContent::Files(vec![PathBuf::from("/tmp/file.txt")]);
/// assert_eq!(files.content_type(), ClipboardContentType::Files);
///
/// // Empty clipboard
/// let empty = ClipboardContent::Empty;
/// assert_eq!(empty.content_type(), ClipboardContentType::Empty);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardContent {
    /// Text content.
    Text(String),
    /// Image data.
    Image(ImageData),
    /// File paths.
    Files(Vec<PathBuf>),
    /// Empty clipboard.
    Empty,
}

impl ClipboardContent {
    /// Get the content type of this clipboard content.
    ///
    /// # Returns
    ///
    /// The [`ClipboardContentType`] representing this content.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::clipboard_types::ClipboardContent;
    /// use codirigent_core::ClipboardContentType;
    ///
    /// let text = ClipboardContent::Text("Hello".to_string());
    /// assert_eq!(text.content_type(), ClipboardContentType::Text);
    /// ```
    pub fn content_type(&self) -> ClipboardContentType {
        match self {
            ClipboardContent::Text(_) => ClipboardContentType::Text,
            ClipboardContent::Image(_) => ClipboardContentType::Image,
            ClipboardContent::Files(_) => ClipboardContentType::Files,
            ClipboardContent::Empty => ClipboardContentType::Empty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // CliType tests

    #[test]
    fn test_cli_type_format_claude_code() {
        let cli = CliType::ClaudeCode;
        let path = Path::new("/tmp/screenshot.png");
        let formatted = cli.format_image_input(path);
        assert_eq!(formatted, "/tmp/screenshot.png");
    }

    #[test]
    fn test_cli_type_format_gemini_cli() {
        let cli = CliType::GeminiCli;
        let path = Path::new("/tmp/screenshot.png");
        let formatted = cli.format_image_input(path);
        assert_eq!(formatted, "@/tmp/screenshot.png");
    }

    #[test]
    fn test_cli_type_format_codex_cli() {
        let cli = CliType::CodexCli;
        let path = Path::new("/tmp/screenshot.png");
        let formatted = cli.format_image_input(path);
        assert_eq!(formatted, "/tmp/screenshot.png");
    }

    #[test]
    fn test_cli_type_format_generic_shell() {
        let cli = CliType::GenericShell;
        let path = Path::new("/tmp/screenshot.png");
        let formatted = cli.format_image_input(path);
        assert_eq!(formatted, "/tmp/screenshot.png");
    }

    #[test]
    fn test_cli_type_detect_from_process_name() {
        // Direct process name matches
        assert_eq!(CliType::detect("claude", None), CliType::ClaudeCode);
        assert_eq!(CliType::detect("Claude", None), CliType::ClaudeCode);
        assert_eq!(CliType::detect("CLAUDE", None), CliType::ClaudeCode);
        assert_eq!(CliType::detect("claude-code", None), CliType::ClaudeCode);

        assert_eq!(CliType::detect("gemini", None), CliType::GeminiCli);
        assert_eq!(CliType::detect("Gemini", None), CliType::GeminiCli);
        assert_eq!(CliType::detect("gemini-cli", None), CliType::GeminiCli);

        assert_eq!(CliType::detect("codex", None), CliType::CodexCli);
        assert_eq!(CliType::detect("Codex", None), CliType::CodexCli);
        assert_eq!(CliType::detect("codex-cli", None), CliType::CodexCli);

        // Unknown processes
        assert_eq!(CliType::detect("bash", None), CliType::GenericShell);
        assert_eq!(CliType::detect("zsh", None), CliType::GenericShell);
        assert_eq!(CliType::detect("python", None), CliType::GenericShell);
    }

    #[test]
    fn test_cli_type_detect_from_command_line() {
        // Process name doesn't match, but command line does
        assert_eq!(
            CliType::detect("node", Some("/usr/bin/claude")),
            CliType::ClaudeCode
        );
        assert_eq!(
            CliType::detect("node", Some("npx claude-code")),
            CliType::ClaudeCode
        );

        assert_eq!(
            CliType::detect("python", Some("gemini-cli run")),
            CliType::GeminiCli
        );

        assert_eq!(
            CliType::detect("node", Some("/path/to/codex")),
            CliType::CodexCli
        );

        // Neither matches
        assert_eq!(
            CliType::detect("node", Some("/usr/bin/npm start")),
            CliType::GenericShell
        );
    }

    #[test]
    fn test_cli_type_default_is_generic_shell() {
        assert_eq!(CliType::default(), CliType::GenericShell);
    }

    #[test]
    fn test_cli_type_serialization() {
        let cli = CliType::GeminiCli;
        let json = serde_json::to_string(&cli).unwrap();
        assert_eq!(json, "\"GeminiCli\"");

        let parsed: CliType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, cli);
    }

    #[test]
    fn test_cli_type_all_variants_serialization() {
        let variants = [
            CliType::ClaudeCode,
            CliType::GeminiCli,
            CliType::CodexCli,
            CliType::GenericShell,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: CliType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_cli_type_equality() {
        assert_eq!(CliType::ClaudeCode, CliType::ClaudeCode);
        assert_ne!(CliType::ClaudeCode, CliType::GeminiCli);
        assert_ne!(CliType::GeminiCli, CliType::CodexCli);
        assert_ne!(CliType::CodexCli, CliType::GenericShell);
    }

    #[test]
    fn test_cli_type_clone_copy() {
        let cli = CliType::GeminiCli;
        let cloned = cli;
        assert_eq!(cli, cloned);
    }

    #[test]
    fn test_cli_type_debug() {
        let cli = CliType::ClaudeCode;
        let debug_str = format!("{:?}", cli);
        assert!(debug_str.contains("ClaudeCode"));
    }

    // ImageFormat tests

    #[test]
    fn test_image_format_extension_png() {
        let format = ImageFormat::Png;
        assert_eq!(format.extension(), "png");
    }

    #[test]
    fn test_image_format_extension_jpeg() {
        let format = ImageFormat::Jpeg;
        assert_eq!(format.extension(), "jpg");
    }

    #[test]
    fn test_image_format_equality() {
        assert_eq!(ImageFormat::Png, ImageFormat::Png);
        assert_eq!(ImageFormat::Jpeg, ImageFormat::Jpeg);
        assert_ne!(ImageFormat::Png, ImageFormat::Jpeg);
    }

    #[test]
    fn test_image_format_clone_copy() {
        let format = ImageFormat::Png;
        let cloned = format;
        assert_eq!(format, cloned);
    }

    #[test]
    fn test_image_format_debug() {
        let format = ImageFormat::Png;
        let debug_str = format!("{:?}", format);
        assert!(debug_str.contains("Png"));

        let format = ImageFormat::Jpeg;
        let debug_str = format!("{:?}", format);
        assert!(debug_str.contains("Jpeg"));
    }

    // ImageData tests

    #[test]
    fn test_image_data_creation() {
        let data = ImageData {
            bytes: vec![0x89, 0x50, 0x4E, 0x47],
            width: 1920,
            height: 1080,
            format: ImageFormat::Png,
        };
        assert_eq!(data.bytes, vec![0x89, 0x50, 0x4E, 0x47]);
        assert_eq!(data.width, 1920);
        assert_eq!(data.height, 1080);
        assert_eq!(data.format, ImageFormat::Png);
    }

    #[test]
    fn test_image_data_jpeg() {
        let data = ImageData {
            bytes: vec![0xFF, 0xD8, 0xFF, 0xE0],
            width: 640,
            height: 480,
            format: ImageFormat::Jpeg,
        };
        assert_eq!(data.format, ImageFormat::Jpeg);
        assert_eq!(data.width, 640);
        assert_eq!(data.height, 480);
    }

    #[test]
    fn test_image_data_clone() {
        let data = ImageData {
            bytes: vec![1, 2, 3, 4],
            width: 100,
            height: 100,
            format: ImageFormat::Png,
        };
        let cloned = data.clone();
        assert_eq!(cloned.bytes, data.bytes);
        assert_eq!(cloned.width, data.width);
        assert_eq!(cloned.height, data.height);
        assert_eq!(cloned.format, data.format);
    }

    #[test]
    fn test_image_data_debug() {
        let data = ImageData {
            bytes: vec![1, 2, 3],
            width: 100,
            height: 100,
            format: ImageFormat::Png,
        };
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("ImageData"));
        assert!(debug_str.contains("width: 100"));
        assert!(debug_str.contains("height: 100"));
        assert!(debug_str.contains("Png"));
    }

    // ClipboardContent tests

    #[test]
    fn test_clipboard_content_text() {
        let content = ClipboardContent::Text("Hello, world!".to_string());
        assert_eq!(content.content_type(), ClipboardContentType::Text);

        if let ClipboardContent::Text(text) = content {
            assert_eq!(text, "Hello, world!");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_clipboard_content_image() {
        let image_data = ImageData {
            bytes: vec![0x89, 0x50, 0x4E, 0x47],
            width: 800,
            height: 600,
            format: ImageFormat::Png,
        };
        let content = ClipboardContent::Image(image_data);
        assert_eq!(content.content_type(), ClipboardContentType::Image);

        if let ClipboardContent::Image(data) = content {
            assert_eq!(data.width, 800);
            assert_eq!(data.height, 600);
        } else {
            panic!("Expected Image variant");
        }
    }

    #[test]
    fn test_clipboard_content_files() {
        let files = vec![
            PathBuf::from("/tmp/file1.txt"),
            PathBuf::from("/tmp/file2.txt"),
            PathBuf::from("/tmp/file3.txt"),
        ];
        let content = ClipboardContent::Files(files.clone());
        assert_eq!(content.content_type(), ClipboardContentType::Files);

        if let ClipboardContent::Files(paths) = content {
            assert_eq!(paths.len(), 3);
            assert_eq!(paths[0], PathBuf::from("/tmp/file1.txt"));
            assert_eq!(paths[1], PathBuf::from("/tmp/file2.txt"));
            assert_eq!(paths[2], PathBuf::from("/tmp/file3.txt"));
        } else {
            panic!("Expected Files variant");
        }
    }

    #[test]
    fn test_clipboard_content_empty() {
        let content = ClipboardContent::Empty;
        assert_eq!(content.content_type(), ClipboardContentType::Empty);
    }

    #[test]
    fn test_clipboard_content_clone() {
        let content = ClipboardContent::Text("Test".to_string());
        let cloned = content.clone();
        assert_eq!(cloned.content_type(), ClipboardContentType::Text);

        if let ClipboardContent::Text(text) = cloned {
            assert_eq!(text, "Test");
        }
    }

    #[test]
    fn test_clipboard_content_debug() {
        let content = ClipboardContent::Text("Hello".to_string());
        let debug_str = format!("{:?}", content);
        assert!(debug_str.contains("Text"));
        assert!(debug_str.contains("Hello"));

        let content = ClipboardContent::Empty;
        let debug_str = format!("{:?}", content);
        assert!(debug_str.contains("Empty"));
    }

    // ClipboardContentType tests

    #[test]
    fn test_clipboard_content_type_equality() {
        assert_eq!(ClipboardContentType::Text, ClipboardContentType::Text);
        assert_eq!(ClipboardContentType::Image, ClipboardContentType::Image);
        assert_eq!(ClipboardContentType::Files, ClipboardContentType::Files);
        assert_eq!(ClipboardContentType::Empty, ClipboardContentType::Empty);

        assert_ne!(ClipboardContentType::Text, ClipboardContentType::Image);
        assert_ne!(ClipboardContentType::Image, ClipboardContentType::Files);
        assert_ne!(ClipboardContentType::Files, ClipboardContentType::Empty);
    }

    #[test]
    fn test_clipboard_content_type_clone_copy() {
        let content_type = ClipboardContentType::Text;
        let cloned = content_type;
        assert_eq!(content_type, cloned);
    }

    #[test]
    fn test_clipboard_content_type_debug() {
        let types = [
            (ClipboardContentType::Text, "Text"),
            (ClipboardContentType::Image, "Image"),
            (ClipboardContentType::Files, "Files"),
            (ClipboardContentType::Empty, "Empty"),
        ];
        for (content_type, expected) in types {
            let debug_str = format!("{:?}", content_type);
            assert!(debug_str.contains(expected));
        }
    }

    // Additional edge case tests

    #[test]
    fn test_cli_type_format_with_spaces_in_path() {
        let cli = CliType::GeminiCli;
        let path = Path::new("/tmp/my screenshots/image 1.png");
        let formatted = cli.format_image_input(path);
        assert_eq!(formatted, "@/tmp/my screenshots/image 1.png");
    }

    #[test]
    fn test_clipboard_content_files_empty_vec() {
        let content = ClipboardContent::Files(vec![]);
        assert_eq!(content.content_type(), ClipboardContentType::Files);

        if let ClipboardContent::Files(paths) = content {
            assert!(paths.is_empty());
        }
    }

    #[test]
    fn test_clipboard_content_text_empty_string() {
        let content = ClipboardContent::Text(String::new());
        assert_eq!(content.content_type(), ClipboardContentType::Text);

        if let ClipboardContent::Text(text) = content {
            assert!(text.is_empty());
        }
    }

    #[test]
    fn test_image_data_empty_bytes() {
        let data = ImageData {
            bytes: vec![],
            width: 0,
            height: 0,
            format: ImageFormat::Png,
        };
        assert!(data.bytes.is_empty());
        assert_eq!(data.width, 0);
        assert_eq!(data.height, 0);
    }

    #[test]
    fn test_cli_type_detect_priority_process_over_cmdline() {
        // Process name takes priority over command line
        // If process name is "gemini", we should get GeminiCli even if cmdline mentions claude
        assert_eq!(
            CliType::detect("gemini", Some("claude related")),
            CliType::GeminiCli
        );
    }

    #[test]
    fn test_clipboard_content_files_single_file() {
        let content = ClipboardContent::Files(vec![PathBuf::from("/tmp/single.txt")]);
        if let ClipboardContent::Files(paths) = content {
            assert_eq!(paths.len(), 1);
            assert_eq!(paths[0], PathBuf::from("/tmp/single.txt"));
        }
    }

    #[test]
    fn test_cli_type_detect_case_insensitive_cmdline() {
        assert_eq!(CliType::detect("node", Some("CLAUDE")), CliType::ClaudeCode);
        assert_eq!(CliType::detect("node", Some("GEMINI")), CliType::GeminiCli);
        assert_eq!(CliType::detect("node", Some("CODEX")), CliType::CodexCli);
    }
}
