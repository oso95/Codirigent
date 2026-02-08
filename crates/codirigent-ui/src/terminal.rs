//! Terminal emulator wrapper using alacritty_terminal.
//!
//! This module provides a high-level wrapper around alacritty_terminal's `Term` type,
//! handling VT100/ANSI escape sequence processing, cursor management, and dirty state
//! tracking for efficient rendering.
//!
//! # Architecture
//!
//! The terminal emulation is built on:
//! - `alacritty_terminal::Term` - The core terminal state machine
//! - `vte::Parser` - VT100/ANSI escape sequence parser
//! - `TerminalEventHandler` - Event listener for terminal events
//!
//! # Example
//!
//! ```rust,ignore
//! use codirigent_ui::terminal::Terminal;
//! use codirigent_core::SessionId;
//!
//! let mut terminal = Terminal::new(24, 80, SessionId(1));
//! terminal.process_output(b"Hello, \x1b[32mWorld\x1b[0m!");
//! let (row, col) = terminal.cursor_position();
//! ```

use alacritty_terminal::event::{Event as TermEvent, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor as VteProcessor;
use codirigent_core::{ClipboardContent, SessionId};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Terminal size configuration.
///
/// Implements `alacritty_terminal::grid::Dimensions` to provide size information
/// to the terminal emulator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalSize {
    /// Number of visible rows.
    pub rows: u16,
    /// Number of visible columns.
    pub cols: u16,
    /// Width of a single character cell in pixels.
    pub cell_width: f32,
    /// Height of a single character cell in pixels.
    pub cell_height: f32,
}

impl TerminalSize {
    /// Create a new terminal size.
    ///
    /// # Arguments
    ///
    /// * `rows` - Number of visible rows
    /// * `cols` - Number of visible columns
    /// * `cell_width` - Width of a single cell in pixels
    /// * `cell_height` - Height of a single cell in pixels
    pub fn new(rows: u16, cols: u16, cell_width: f32, cell_height: f32) -> Self {
        Self {
            rows,
            cols,
            cell_width,
            cell_height,
        }
    }

    /// Create a terminal size with default cell dimensions.
    ///
    /// Uses 8.0 x 16.0 pixels as default cell size.
    pub fn with_default_cells(rows: u16, cols: u16) -> Self {
        Self::new(rows, cols, 8.0, 16.0)
    }
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self::with_default_cells(24, 80)
    }
}

impl Dimensions for TerminalSize {
    fn columns(&self) -> usize {
        self.cols as usize
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn total_lines(&self) -> usize {
        // For now, total lines equals screen lines (no scrollback)
        // Scrollback is managed by alacritty_terminal internally
        self.rows as usize
    }
}

/// Event handler for terminal events.
///
/// Receives events from alacritty_terminal such as title changes, bell rings,
/// and clipboard operations. Optionally integrates with the system clipboard
/// for copy/paste operations, and can forward PTY write requests to the session.
pub struct TerminalEventHandler {
    session_id: SessionId,
    /// Optional clipboard provider for system clipboard integration.
    clipboard: Option<Arc<dyn crate::smart_clipboard::SmartClipboardProvider>>,
    /// Optional channel for sending PTY write requests back to the session.
    pty_writer: Option<mpsc::UnboundedSender<Vec<u8>>>,
}

impl TerminalEventHandler {
    /// Create a new event handler for the given session.
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            clipboard: None,
            pty_writer: None,
        }
    }

    /// Create a new event handler with clipboard integration.
    pub fn with_clipboard(
        session_id: SessionId,
        clipboard: Arc<dyn crate::smart_clipboard::SmartClipboardProvider>,
    ) -> Self {
        Self {
            session_id,
            clipboard: Some(clipboard),
            pty_writer: None,
        }
    }

    /// Create a new event handler with full integration.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session this handler is associated with
    /// * `clipboard` - Clipboard provider for copy/paste operations
    /// * `pty_writer` - Channel sender for forwarding PTY write requests
    pub fn with_full_integration(
        session_id: SessionId,
        clipboard: Arc<dyn crate::smart_clipboard::SmartClipboardProvider>,
        pty_writer: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Self {
        Self {
            session_id,
            clipboard: Some(clipboard),
            pty_writer: Some(pty_writer),
        }
    }

    /// Get the session ID this handler is associated with.
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
}

impl EventListener for TerminalEventHandler {
    fn send_event(&self, event: TermEvent) {
        match event {
            TermEvent::Title(title) => {
                debug!(
                    session_id = %self.session_id,
                    %title,
                    "Terminal title changed"
                );
            }
            TermEvent::ResetTitle => {
                debug!(
                    session_id = %self.session_id,
                    "Terminal title reset"
                );
            }
            TermEvent::Bell => {
                debug!(session_id = %self.session_id, "Terminal bell");
            }
            TermEvent::ClipboardStore(clipboard_type, text) => {
                debug!(
                    session_id = %self.session_id,
                    ?clipboard_type,
                    text_len = text.len(),
                    "Clipboard store request"
                );
                // Write text to system clipboard
                if let Some(ref clipboard) = self.clipboard {
                    if let Err(e) = clipboard.write_text(text) {
                        warn!(
                            session_id = %self.session_id,
                            error = %e,
                            "Failed to store text in clipboard"
                        );
                    }
                }
            }
            TermEvent::ClipboardLoad(clipboard_type, _format) => {
                debug!(
                    session_id = %self.session_id,
                    ?clipboard_type,
                    "Clipboard load request"
                );
                // Note: ClipboardLoad is typically used by terminal applications to request
                // clipboard content. The response is usually sent via the terminal's input
                // (PTY write). For now we just log it - full implementation would require
                // a callback mechanism to send the clipboard content back to the terminal.
                if let Some(ref clipboard) = self.clipboard {
                    match clipboard.read_content() {
                        Ok(ClipboardContent::Text(text)) => {
                            debug!(
                                session_id = %self.session_id,
                                text_len = text.len(),
                                "Clipboard text available for terminal"
                            );
                            // The actual response would need to be written to the PTY
                            // via OSC 52 response sequence. This requires PTY write access.
                        }
                        Ok(other) => {
                            debug!(
                                session_id = %self.session_id,
                                content_type = ?std::mem::discriminant(&other),
                                "Clipboard has non-text content"
                            );
                        }
                        Err(e) => {
                            warn!(
                                session_id = %self.session_id,
                                error = %e,
                                "Failed to load clipboard content"
                            );
                        }
                    }
                }
            }
            TermEvent::ColorRequest(index, _format) => {
                debug!(
                    session_id = %self.session_id,
                    index,
                    "Color request"
                );
            }
            TermEvent::PtyWrite(text) => {
                debug!(
                    session_id = %self.session_id,
                    text_len = text.len(),
                    "PTY write request"
                );
                // Forward to PTY via the channel
                if let Some(ref sender) = self.pty_writer {
                    let bytes = text.into_bytes();
                    if let Err(e) = sender.send(bytes) {
                        warn!(
                            session_id = %self.session_id,
                            error = %e,
                            "Failed to forward PTY write request"
                        );
                    }
                }
            }
            TermEvent::TextAreaSizeRequest(_format) => {
                debug!(
                    session_id = %self.session_id,
                    "Text area size request"
                );
            }
            TermEvent::CursorBlinkingChange => {
                debug!(
                    session_id = %self.session_id,
                    "Cursor blinking state changed"
                );
            }
            TermEvent::MouseCursorDirty => {
                // Mouse cursor shape may need update
            }
            TermEvent::Wakeup => {
                // New terminal content available
            }
            TermEvent::Exit => {
                debug!(session_id = %self.session_id, "Terminal exit requested");
            }
            TermEvent::ChildExit(code) => {
                debug!(
                    session_id = %self.session_id,
                    exit_code = code,
                    "Child process exited"
                );
            }
        }
    }
}

/// Terminal emulator wrapper.
///
/// Wraps `alacritty_terminal::Term` with additional state tracking for efficient
/// rendering and integration with the Codirigent session system.
pub struct Terminal {
    /// The underlying alacritty terminal.
    term: Term<TerminalEventHandler>,
    /// VTE escape sequence processor.
    processor: VteProcessor,
    /// Whether the terminal content has changed since last render.
    dirty: bool,
    /// The session this terminal belongs to.
    session_id: SessionId,
    /// Current terminal size.
    size: TerminalSize,
}

impl Terminal {
    /// Create a new terminal with the given dimensions.
    ///
    /// # Arguments
    ///
    /// * `rows` - Number of visible rows
    /// * `cols` - Number of visible columns
    /// * `session_id` - The session this terminal belongs to
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use codirigent_ui::terminal::Terminal;
    /// use codirigent_core::SessionId;
    ///
    /// let terminal = Terminal::new(24, 80, SessionId(1));
    /// ```
    pub fn new(rows: u16, cols: u16, session_id: SessionId) -> Self {
        let size = TerminalSize::with_default_cells(rows, cols);
        let config = Config::default();
        let handler = TerminalEventHandler::new(session_id);

        let term = Term::new(config, &size, handler);
        let processor = VteProcessor::new();

        debug!(
            session_id = %session_id,
            rows,
            cols,
            "Created new terminal"
        );

        Self {
            term,
            processor,
            dirty: true,
            session_id,
            size,
        }
    }

    /// Create a terminal with custom cell dimensions.
    ///
    /// # Arguments
    ///
    /// * `size` - Terminal size configuration
    /// * `session_id` - The session this terminal belongs to
    pub fn with_size(size: TerminalSize, session_id: SessionId) -> Self {
        let config = Config::default();
        let handler = TerminalEventHandler::new(session_id);

        let term = Term::new(config, &size, handler);
        let processor = VteProcessor::new();

        Self {
            term,
            processor,
            dirty: true,
            session_id,
            size,
        }
    }

    /// Process incoming output data from the PTY.
    ///
    /// Parses VT100/ANSI escape sequences and updates the terminal state.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw bytes from the PTY output
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// terminal.process_output(b"\x1b[2J\x1b[H"); // Clear screen and home cursor
    /// terminal.process_output(b"Hello, World!");
    /// ```
    pub fn process_output(&mut self, data: &[u8]) {
        self.processor.advance(&mut self.term, data);
        self.dirty = true;
    }

    /// Process output in chunks for better performance.
    ///
    /// This is more efficient for large amounts of data as it processes
    /// in batches rather than byte-by-byte.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw bytes from the PTY output
    /// * `chunk_size` - Number of bytes to process per batch
    pub fn process_output_chunked(&mut self, data: &[u8], chunk_size: usize) {
        for chunk in data.chunks(chunk_size) {
            self.processor.advance(&mut self.term, chunk);
        }
        self.dirty = true;
    }

    /// Resize the terminal to new dimensions.
    ///
    /// # Arguments
    ///
    /// * `rows` - New number of visible rows
    /// * `cols` - New number of visible columns
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.size = TerminalSize::new(rows, cols, self.size.cell_width, self.size.cell_height);
        self.term.resize(self.size);
        self.dirty = true;

        debug!(
            session_id = %self.session_id,
            rows,
            cols,
            "Terminal resized"
        );
    }

    /// Resize with custom cell dimensions.
    ///
    /// # Arguments
    ///
    /// * `size` - New terminal size configuration
    pub fn resize_with_cells(&mut self, size: TerminalSize) {
        self.size = size;
        self.term.resize(size);
        self.dirty = true;
    }

    /// Check if the terminal needs to be redrawn.
    ///
    /// Returns `true` if terminal content has changed since the last render.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the terminal as clean after rendering.
    ///
    /// Call this after successfully rendering the terminal to prevent
    /// unnecessary redraws.
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Get the cursor position as (row, column).
    ///
    /// Row and column are 0-indexed.
    pub fn cursor_position(&self) -> (usize, usize) {
        let cursor = &self.term.grid().cursor;
        (cursor.point.line.0 as usize, cursor.point.column.0)
    }

    /// Check if the cursor should be visible.
    ///
    /// Returns `false` if the terminal is in cursor-hidden mode.
    pub fn cursor_visible(&self) -> bool {
        self.term.mode().contains(TermMode::SHOW_CURSOR)
    }

    /// Get the current terminal mode flags.
    pub fn mode(&self) -> TermMode {
        *self.term.mode()
    }

    /// Access the underlying alacritty term for rendering.
    ///
    /// Use `renderable_content()` on the returned term to get cells for rendering.
    pub fn term(&self) -> &Term<TerminalEventHandler> {
        &self.term
    }

    /// Get mutable access to the underlying term.
    ///
    /// Use with caution - direct modifications bypass dirty tracking.
    pub fn term_mut(&mut self) -> &mut Term<TerminalEventHandler> {
        self.dirty = true;
        &mut self.term
    }

    /// Get the session ID this terminal belongs to.
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Get the current terminal size.
    pub fn size(&self) -> TerminalSize {
        self.size
    }

    /// Get the number of rows.
    pub fn rows(&self) -> u16 {
        self.size.rows
    }

    /// Get the number of columns.
    pub fn cols(&self) -> u16 {
        self.size.cols
    }

    /// Calculate terminal size from pixel bounds.
    ///
    /// Given pixel dimensions, calculates how many rows and columns can fit
    /// using the current cell dimensions.
    ///
    /// # Arguments
    ///
    /// * `width_px` - Width in pixels
    /// * `height_px` - Height in pixels
    ///
    /// # Returns
    ///
    /// A tuple of (rows, cols) with minimum of 1 for each dimension.
    pub fn calculate_size_from_pixels(&self, width_px: f32, height_px: f32) -> (u16, u16) {
        let cols = (width_px / self.size.cell_width) as u16;
        let rows = (height_px / self.size.cell_height) as u16;
        (rows.max(1), cols.max(1))
    }

    /// Resize terminal to fit within pixel bounds.
    ///
    /// Convenience method that calculates the appropriate row/col count
    /// from pixel dimensions and resizes the terminal.
    ///
    /// # Arguments
    ///
    /// * `width_px` - Width in pixels
    /// * `height_px` - Height in pixels
    pub fn resize_to_fit(&mut self, width_px: f32, height_px: f32) {
        let (rows, cols) = self.calculate_size_from_pixels(width_px, height_px);
        self.resize(rows, cols);
    }

    /// Get the cell dimensions.
    pub fn cell_dimensions(&self) -> (f32, f32) {
        (self.size.cell_width, self.size.cell_height)
    }

    /// Check if application cursor mode is enabled.
    ///
    /// When enabled, arrow keys send different escape sequences.
    pub fn app_cursor_mode(&self) -> bool {
        self.term.mode().contains(TermMode::APP_CURSOR)
    }

    /// Check if bracketed paste mode is enabled.
    ///
    /// When enabled, pasted text should be wrapped with escape sequences.
    pub fn bracketed_paste_mode(&self) -> bool {
        self.term.mode().contains(TermMode::BRACKETED_PASTE)
    }

    /// Clear the terminal screen while preserving the current line (prompt).
    pub fn clear(&mut self) {
        use alacritty_terminal::index::{Column, Line};
        use alacritty_terminal::vte::ansi::{ClearMode, Handler};

        self.term.clear_screen(ClearMode::Saved);

        let cursor = self.term.grid().cursor.point;
        self.term.grid_mut().reset_region(..cursor.line);

        let cols = self.term.grid().columns();
        let line: Vec<_> = self.term.grid()[cursor.line][..Column(cols)]
            .iter()
            .cloned()
            .enumerate()
            .collect();

        for (i, cell) in line {
            self.term.grid_mut()[Line(0)][Column(i)] = cell;
        }

        self.term.grid_mut().cursor.point =
            alacritty_terminal::index::Point::new(Line(0), cursor.column);
        let new_cursor = self.term.grid().cursor.point;

        if (new_cursor.line.0 as usize) < self.term.screen_lines() - 1 {
            self.term.grid_mut().reset_region((new_cursor.line + 1)..);
        }

        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_size_default() {
        let size = TerminalSize::default();
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
        assert_eq!(size.cell_width, 8.0);
        assert_eq!(size.cell_height, 16.0);
    }

    #[test]
    fn test_terminal_size_dimensions() {
        let size = TerminalSize::new(30, 100, 10.0, 20.0);
        assert_eq!(size.columns(), 100);
        assert_eq!(size.screen_lines(), 30);
        assert_eq!(size.total_lines(), 30);
    }

    #[test]
    fn test_terminal_creation() {
        let terminal = Terminal::new(24, 80, SessionId(1));
        assert!(terminal.is_dirty());
        assert_eq!(terminal.cursor_position(), (0, 0));
        assert_eq!(terminal.rows(), 24);
        assert_eq!(terminal.cols(), 80);
        assert_eq!(terminal.session_id(), SessionId(1));
    }

    #[test]
    fn test_terminal_with_size() {
        let size = TerminalSize::new(48, 120, 9.0, 18.0);
        let terminal = Terminal::with_size(size, SessionId(2));
        assert_eq!(terminal.rows(), 48);
        assert_eq!(terminal.cols(), 120);
        assert_eq!(terminal.size().cell_width, 9.0);
    }

    #[test]
    fn test_process_output_marks_dirty() {
        let mut terminal = Terminal::new(24, 80, SessionId(1));
        terminal.mark_clean();
        assert!(!terminal.is_dirty());

        terminal.process_output(b"Hello, World!");
        assert!(terminal.is_dirty());
    }

    #[test]
    fn test_resize() {
        let mut terminal = Terminal::new(24, 80, SessionId(1));
        terminal.mark_clean();

        terminal.resize(48, 120);
        assert!(terminal.is_dirty());
        assert_eq!(terminal.rows(), 48);
        assert_eq!(terminal.cols(), 120);
    }

    #[test]
    fn test_resize_with_cells() {
        let mut terminal = Terminal::new(24, 80, SessionId(1));
        let new_size = TerminalSize::new(30, 100, 10.0, 20.0);

        terminal.resize_with_cells(new_size);
        assert_eq!(terminal.size().cell_width, 10.0);
        assert_eq!(terminal.size().cell_height, 20.0);
    }

    #[test]
    fn test_cursor_visibility() {
        let terminal = Terminal::new(24, 80, SessionId(1));
        // Default state should have visible cursor
        assert!(terminal.cursor_visible());
    }

    #[test]
    fn test_process_escape_sequences() {
        let mut terminal = Terminal::new(24, 80, SessionId(1));

        // Move cursor to position (5, 10) using ANSI escape sequence
        // ESC [ row ; col H
        terminal.process_output(b"\x1b[6;11H");

        let (row, col) = terminal.cursor_position();
        // ANSI coordinates are 1-indexed, our return is 0-indexed
        assert_eq!(row, 5);
        assert_eq!(col, 10);
    }

    #[test]
    fn test_chunked_processing() {
        let mut terminal = Terminal::new(24, 80, SessionId(1));

        // Process a longer string in chunks
        let data = b"Hello, World! This is a test of chunked processing.";
        terminal.process_output_chunked(data, 10);

        assert!(terminal.is_dirty());
    }

    #[test]
    fn test_event_handler_session_id() {
        let handler = TerminalEventHandler::new(SessionId(42));
        assert_eq!(handler.session_id(), SessionId(42));
    }

    #[test]
    fn test_calculate_size_from_pixels() {
        let terminal = Terminal::new(24, 80, SessionId(1));
        // Default cell size is 8x16
        let (rows, cols) = terminal.calculate_size_from_pixels(800.0, 480.0);
        assert_eq!(cols, 100); // 800 / 8 = 100
        assert_eq!(rows, 30); // 480 / 16 = 30
    }

    #[test]
    fn test_calculate_size_minimum() {
        let terminal = Terminal::new(24, 80, SessionId(1));
        // Very small dimensions should return minimum 1x1
        let (rows, cols) = terminal.calculate_size_from_pixels(1.0, 1.0);
        assert_eq!(rows, 1);
        assert_eq!(cols, 1);
    }

    #[test]
    fn test_resize_to_fit() {
        let mut terminal = Terminal::new(24, 80, SessionId(1));
        terminal.resize_to_fit(640.0, 400.0);
        assert_eq!(terminal.cols(), 80); // 640 / 8 = 80
        assert_eq!(terminal.rows(), 25); // 400 / 16 = 25
    }

    #[test]
    fn test_cell_dimensions() {
        let terminal = Terminal::new(24, 80, SessionId(1));
        let (width, height) = terminal.cell_dimensions();
        assert_eq!(width, 8.0);
        assert_eq!(height, 16.0);
    }

    #[test]
    fn test_app_cursor_mode() {
        let terminal = Terminal::new(24, 80, SessionId(1));
        // Default should be off
        assert!(!terminal.app_cursor_mode());
    }

    #[test]
    fn test_bracketed_paste_mode() {
        let terminal = Terminal::new(24, 80, SessionId(1));
        // Default should be off
        assert!(!terminal.bracketed_paste_mode());
    }
}
