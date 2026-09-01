//! Clipboard operations for terminal.
//!
//! This module provides copy and paste functionality for terminal content,
//! integrating with the system clipboard.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::Term;

/// Copy selected text from terminal grid.
///
/// Extracts text from the terminal grid between the specified start and end
/// positions. The selection is inclusive of both endpoints.
///
/// # Arguments
///
/// * `term` - The terminal instance to copy from
/// * `start` - Start position as (grid_line, col)
/// * `end` - End position as (grid_line, col)
///
/// # Returns
///
/// The selected text as a string, with trailing whitespace trimmed from lines.
pub fn copy_selection<T>(term: &Term<T>, start: (i32, usize), end: (i32, usize)) -> String {
    let mut text = String::new();

    // Normalize selection (ensure start <= end)
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    let (start_line, start_col) = start;
    let (end_line, end_col) = end;

    let grid = term.grid();
    let total_cols = grid.columns();
    let start_line = start_line.max(term.topmost_line().0);
    let end_line = end_line.min(term.bottommost_line().0);

    if start_line > end_line {
        return String::new();
    }

    for line_idx in start_line..=end_line {
        let line = &grid[Line(line_idx)];
        let col_start = if line_idx == start_line { start_col } else { 0 };
        let col_end = if line_idx == end_line {
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

        // Only add newline between lines if the current line does NOT wrap
        // to the next row. Wrapped lines are continuations of the same
        // logical line and should be joined without a newline, matching
        // the behavior of standard terminal emulators.
        if line_idx < end_line {
            let last_col = Column(total_cols.saturating_sub(1));
            let is_wrapped = line[last_col].flags.contains(Flags::WRAPLINE);
            if !is_wrapped {
                text.push('\n');
            }
        }
    }

    text.trim_end().to_string()
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
}
