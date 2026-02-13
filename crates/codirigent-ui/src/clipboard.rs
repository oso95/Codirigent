//! Clipboard operations for terminal.
//!
//! This module provides copy and paste functionality for terminal content,
//! integrating with the system clipboard.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::Term;

/// Copy selected text from terminal grid.
///
/// Extracts text from the terminal grid between the specified start and end
/// positions. The selection is inclusive of both endpoints.
///
/// # Arguments
///
/// * `term` - The terminal instance to copy from
/// * `start` - Start position as (row, col)
/// * `end` - End position as (row, col)
///
/// # Returns
///
/// The selected text as a string, with trailing whitespace trimmed from lines.
pub fn copy_selection<T>(term: &Term<T>, start: (usize, usize), end: (usize, usize)) -> String {
    let mut text = String::new();

    // Normalize selection (ensure start <= end)
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let (start_row, start_col) = start;
    let (end_row, end_col) = end;

    let grid = term.grid();
    let total_lines = grid.screen_lines();
    let total_cols = grid.columns();

    for row in start_row..=end_row {
        if row >= total_lines {
            break;
        }

        let line = &grid[Line(row as i32)];
        let col_start = if row == start_row { start_col } else { 0 };
        let col_end = if row == end_row {
            end_col.min(total_cols.saturating_sub(1))
        } else {
            total_cols.saturating_sub(1)
        };

        let mut line_text = String::new();
        for col in col_start..=col_end {
            if col >= total_cols {
                break;
            }

            let cell = &line[Column(col)];
            line_text.push(cell.c);
        }

        // Trim trailing whitespace from the line
        let trimmed = line_text.trim_end();
        text.push_str(trimmed);

        // Add newline between lines (but not after the last line)
        if row < end_row && !trimmed.is_empty() {
            text.push('\n');
        } else if row < end_row && trimmed.is_empty() {
            // Preserve empty lines in multi-line selections
            text.push('\n');
        }
    }

    text.trim_end().to_string()
}

/// Trait for clipboard operations.
///
/// This trait abstracts clipboard access to allow for testing and
/// platform-specific implementations.
pub trait ClipboardProvider: Send + Sync {
    /// Write text to the clipboard.
    fn write(&self, text: String) -> anyhow::Result<()>;

    /// Read text from the clipboard.
    fn read(&self) -> anyhow::Result<Option<String>>;
}

/// A no-op clipboard provider for testing.
#[derive(Debug, Default)]
pub struct NoopClipboard;

impl ClipboardProvider for NoopClipboard {
    fn write(&self, _text: String) -> anyhow::Result<()> {
        Ok(())
    }

    fn read(&self) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
}

/// An in-memory clipboard provider for testing.
#[derive(Debug, Default)]
pub struct TestClipboard {
    content: std::sync::Mutex<Option<String>>,
}

impl TestClipboard {
    /// Create a new test clipboard.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a test clipboard with initial content.
    pub fn with_content(content: impl Into<String>) -> Self {
        Self {
            content: std::sync::Mutex::new(Some(content.into())),
        }
    }
}

impl ClipboardProvider for TestClipboard {
    fn write(&self, text: String) -> anyhow::Result<()> {
        let mut content = self.content.lock().expect("TestClipboard mutex poisoned");
        *content = Some(text);
        Ok(())
    }

    fn read(&self) -> anyhow::Result<Option<String>> {
        let content = self.content.lock().expect("TestClipboard mutex poisoned");
        Ok(content.clone())
    }
}

/// Prepare text for pasting into terminal.
///
/// Handles bracketed paste mode by wrapping the text with the appropriate
/// escape sequences.
///
/// # Arguments
///
/// * `text` - The text to paste
/// * `bracketed_paste` - Whether bracketed paste mode is enabled
///
/// # Returns
///
/// The text prepared for sending to the PTY.
pub fn prepare_paste(text: &str, bracketed_paste: bool) -> Vec<u8> {
    if bracketed_paste {
        // Wrap in bracketed paste sequences
        let mut result = Vec::with_capacity(text.len() + 12);
        result.extend_from_slice(b"\x1b[200~");
        result.extend_from_slice(text.as_bytes());
        result.extend_from_slice(b"\x1b[201~");
        result
    } else {
        // Convert newlines to carriage returns for terminal
        text.replace('\n', "\r").into_bytes()
    }
}

/// Sanitize text for safe pasting.
///
/// Removes potentially dangerous control characters that could
/// execute unintended commands.
///
/// # Arguments
///
/// * `text` - The text to sanitize
///
/// # Returns
///
/// The sanitized text safe for pasting.
pub fn sanitize_paste(text: &str) -> String {
    text.chars()
        .filter(|c| {
            // Allow printable characters, newlines, and tabs
            c.is_ascii_graphic() || c.is_ascii_whitespace() || !c.is_ascii() // Allow non-ASCII (unicode)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_paste_without_bracketed() {
        let text = "hello\nworld";
        let result = prepare_paste(text, false);
        assert_eq!(result, b"hello\rworld");
    }

    #[test]
    fn test_prepare_paste_with_bracketed() {
        let text = "hello";
        let result = prepare_paste(text, true);
        assert_eq!(result, b"\x1b[200~hello\x1b[201~");
    }

    #[test]
    fn test_sanitize_paste() {
        // Remove control characters but keep printable
        let text = "hello\x03world"; // Contains Ctrl-C
        let result = sanitize_paste(text);
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn test_sanitize_paste_keeps_newlines() {
        let text = "hello\nworld\ttab";
        let result = sanitize_paste(text);
        assert_eq!(result, "hello\nworld\ttab");
    }

    #[test]
    fn test_sanitize_paste_unicode() {
        let text = "hello \u{4E2D}\u{6587}"; // Chinese characters
        let result = sanitize_paste(text);
        assert_eq!(result, "hello \u{4E2D}\u{6587}");
    }

    #[test]
    fn test_test_clipboard() {
        let clipboard = TestClipboard::new();

        // Initially empty
        assert_eq!(clipboard.read().unwrap(), None);

        // Write and read back
        clipboard.write("test".to_string()).unwrap();
        assert_eq!(clipboard.read().unwrap(), Some("test".to_string()));

        // Overwrite
        clipboard.write("new".to_string()).unwrap();
        assert_eq!(clipboard.read().unwrap(), Some("new".to_string()));
    }

    #[test]
    fn test_test_clipboard_with_content() {
        let clipboard = TestClipboard::with_content("initial");
        assert_eq!(clipboard.read().unwrap(), Some("initial".to_string()));
    }

    #[test]
    fn test_noop_clipboard() {
        let clipboard = NoopClipboard;
        clipboard.write("test".to_string()).unwrap();
        assert_eq!(clipboard.read().unwrap(), None);
    }
}
