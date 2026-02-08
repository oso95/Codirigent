//! Mouse input handling for terminal.
//!
//! This module provides mouse event translation and selection handling
//! for terminal emulation.

use super::keyboard::TerminalModifiers;
use alacritty_terminal::term::TermMode;

/// Mouse button types for terminal mouse events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMouseButton {
    /// Left mouse button.
    Left,
    /// Middle mouse button.
    Middle,
    /// Right mouse button.
    Right,
    /// Scroll wheel up.
    WheelUp,
    /// Scroll wheel down.
    WheelDown,
}

/// Mouse event for terminal.
///
/// Represents a mouse event that can be converted to terminal escape sequences.
#[derive(Debug, Clone)]
pub struct TerminalMouseEvent {
    /// The mouse button involved.
    pub button: TerminalMouseButton,
    /// Row position (0-indexed).
    pub row: usize,
    /// Column position (0-indexed).
    pub col: usize,
    /// Whether the button is pressed (true) or released (false).
    pub pressed: bool,
    /// Active modifiers.
    pub modifiers: TerminalModifiers,
}

impl TerminalMouseEvent {
    /// Create a new mouse event.
    pub fn new(button: TerminalMouseButton, row: usize, col: usize, pressed: bool) -> Self {
        Self {
            button,
            row,
            col,
            pressed,
            modifiers: TerminalModifiers::default(),
        }
    }

    /// Create a mouse event with modifiers.
    pub fn with_modifiers(
        button: TerminalMouseButton,
        row: usize,
        col: usize,
        pressed: bool,
        modifiers: TerminalModifiers,
    ) -> Self {
        Self {
            button,
            row,
            col,
            pressed,
            modifiers,
        }
    }

    /// Convert to terminal escape sequence.
    ///
    /// Returns the escape sequence bytes if mouse reporting is enabled,
    /// or `None` if the terminal mode doesn't support mouse events.
    pub fn to_bytes(&self, mode: TermMode) -> Option<Vec<u8>> {
        // Check if any mouse mode is enabled
        if !mode.contains(TermMode::MOUSE_REPORT_CLICK)
            && !mode.contains(TermMode::MOUSE_MOTION)
            && !mode.contains(TermMode::MOUSE_DRAG)
        {
            return None;
        }

        let mut modifier_code = 0u8;
        if self.modifiers.shift {
            modifier_code |= 4;
        }
        if self.modifiers.alt {
            modifier_code |= 8;
        }
        if self.modifiers.control {
            modifier_code |= 16;
        }

        // SGR extended mouse mode (preferred, supports coordinates > 223)
        // In SGR mode, the button code always indicates the actual button,
        // and press/release is distinguished by the suffix ('M' vs 'm')
        if mode.contains(TermMode::SGR_MOUSE) {
            let button_code = match self.button {
                TerminalMouseButton::Left => 0,
                TerminalMouseButton::Middle => 1,
                TerminalMouseButton::Right => 2,
                TerminalMouseButton::WheelUp => 64,
                TerminalMouseButton::WheelDown => 65,
            };
            let code = button_code + modifier_code;
            let suffix = if self.pressed { 'M' } else { 'm' };
            Some(format!("\x1b[<{};{};{}{}", code, self.col + 1, self.row + 1, suffix).into_bytes())
        } else {
            // For non-SGR modes (UTF-8 and normal X10), button code 3 means "release"
            let button_code = match (self.button, self.pressed) {
                (TerminalMouseButton::Left, true) => 0,
                (TerminalMouseButton::Middle, true) => 1,
                (TerminalMouseButton::Right, true) => 2,
                (_, false) => 3, // Release (no way to indicate which button in X10 mode)
                (TerminalMouseButton::WheelUp, _) => 64,
                (TerminalMouseButton::WheelDown, _) => 65,
            };
            let code = button_code + modifier_code;

            if mode.contains(TermMode::UTF8_MOUSE) {
                // UTF-8 mouse mode (supports larger coordinates)
                let mut bytes = vec![0x1b, b'[', b'M'];
                bytes.push((code + 32) as u8);
                let col = self.col + 1 + 32;
                let row = self.row + 1 + 32;
                if col < 128 {
                    bytes.push(col as u8);
                } else {
                    bytes.push(0xC0 | ((col >> 6) & 0x1F) as u8);
                    bytes.push(0x80 | (col & 0x3F) as u8);
                }
                if row < 128 {
                    bytes.push(row as u8);
                } else {
                    bytes.push(0xC0 | ((row >> 6) & 0x1F) as u8);
                    bytes.push(0x80 | (row & 0x3F) as u8);
                }
                Some(bytes)
            } else {
                // Normal mouse mode (X10 compatible, max 223 for coordinates)
                Some(vec![
                    0x1b,
                    b'[',
                    b'M',
                    (code + 32) as u8,
                    ((self.col + 1 + 32) as u8).min(255),
                    ((self.row + 1 + 32) as u8).min(255),
                ])
            }
        }
    }
}

/// Terminal mouse handler for managing selection state.
///
/// Tracks mouse selection state and converts pixel positions to cell positions.
#[derive(Debug)]
pub struct TerminalMouseHandler {
    /// Start of selection (row, col).
    pub selection_start: Option<(usize, usize)>,
    /// End of selection (row, col).
    pub selection_end: Option<(usize, usize)>,
    /// Whether a selection is in progress.
    pub is_selecting: bool,
    /// Width of a cell in pixels.
    cell_width: f32,
    /// Height of a cell in pixels.
    cell_height: f32,
}

impl TerminalMouseHandler {
    /// Create a new mouse handler with the given cell dimensions.
    pub fn new(cell_width: f32, cell_height: f32) -> Self {
        Self {
            selection_start: None,
            selection_end: None,
            is_selecting: false,
            cell_width,
            cell_height,
        }
    }

    /// Update cell dimensions (e.g., after font size change).
    pub fn set_cell_size(&mut self, width: f32, height: f32) {
        self.cell_width = width;
        self.cell_height = height;
    }

    /// Get the current cell dimensions.
    pub fn cell_size(&self) -> (f32, f32) {
        (self.cell_width, self.cell_height)
    }

    /// Convert pixel position to cell position.
    ///
    /// Returns (row, col) tuple.
    pub fn pixel_to_cell(&self, x: f32, y: f32) -> (usize, usize) {
        let col = (x / self.cell_width).max(0.0) as usize;
        let row = (y / self.cell_height).max(0.0) as usize;
        (row, col)
    }

    /// Start a new selection at the given pixel position.
    pub fn start_selection(&mut self, x: f32, y: f32) {
        let cell = self.pixel_to_cell(x, y);
        self.selection_start = Some(cell);
        self.selection_end = Some(cell);
        self.is_selecting = true;
    }

    /// Update the selection end position.
    pub fn update_selection(&mut self, x: f32, y: f32) {
        if self.is_selecting {
            self.selection_end = Some(self.pixel_to_cell(x, y));
        }
    }

    /// End the current selection.
    pub fn end_selection(&mut self) {
        self.is_selecting = false;
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
        self.is_selecting = false;
    }

    /// Check if there is an active selection.
    pub fn has_selection(&self) -> bool {
        self.selection_start.is_some() && self.selection_end.is_some()
    }

    /// Get the normalized selection bounds (start <= end).
    ///
    /// Returns `((start_row, start_col), (end_row, end_col))` where start <= end.
    pub fn selection_bounds(&self) -> Option<((usize, usize), (usize, usize))> {
        let start = self.selection_start?;
        let end = self.selection_end?;
        Some(if start <= end {
            (start, end)
        } else {
            (end, start)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_event_sgr_mode() {
        let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        let event = TerminalMouseEvent::new(TerminalMouseButton::Left, 5, 10, true);
        let bytes = event.to_bytes(mode);
        assert_eq!(bytes, Some(b"\x1b[<0;11;6M".to_vec()));
    }

    #[test]
    fn test_mouse_event_sgr_release() {
        let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        let event = TerminalMouseEvent::new(TerminalMouseButton::Left, 5, 10, false);
        let bytes = event.to_bytes(mode);
        assert_eq!(bytes, Some(b"\x1b[<0;11;6m".to_vec()));
    }

    #[test]
    fn test_mouse_event_normal_mode() {
        let mode = TermMode::MOUSE_REPORT_CLICK;
        let event = TerminalMouseEvent::new(TerminalMouseButton::Left, 0, 0, true);
        let bytes = event.to_bytes(mode);
        assert_eq!(bytes, Some(vec![0x1b, b'[', b'M', 32, 33, 33]));
    }

    #[test]
    fn test_mouse_event_no_mode() {
        let mode = TermMode::empty();
        let event = TerminalMouseEvent::new(TerminalMouseButton::Left, 0, 0, true);
        assert_eq!(event.to_bytes(mode), None);
    }

    #[test]
    fn test_mouse_handler_pixel_to_cell() {
        let handler = TerminalMouseHandler::new(10.0, 20.0);
        assert_eq!(handler.pixel_to_cell(0.0, 0.0), (0, 0));
        assert_eq!(handler.pixel_to_cell(15.0, 25.0), (1, 1));
        assert_eq!(handler.pixel_to_cell(35.0, 65.0), (3, 3));
    }

    #[test]
    fn test_mouse_handler_selection() {
        let mut handler = TerminalMouseHandler::new(10.0, 20.0);

        assert!(!handler.has_selection());

        handler.start_selection(5.0, 10.0);
        assert!(handler.has_selection());
        assert!(handler.is_selecting);
        assert_eq!(handler.selection_start, Some((0, 0)));

        handler.update_selection(35.0, 65.0);
        assert_eq!(handler.selection_end, Some((3, 3)));

        handler.end_selection();
        assert!(!handler.is_selecting);
        assert!(handler.has_selection());

        let bounds = handler.selection_bounds();
        assert_eq!(bounds, Some(((0, 0), (3, 3))));

        handler.clear_selection();
        assert!(!handler.has_selection());
    }

    #[test]
    fn test_mouse_handler_selection_bounds_normalized() {
        let mut handler = TerminalMouseHandler::new(10.0, 20.0);

        handler.start_selection(35.0, 65.0);
        handler.update_selection(5.0, 10.0);

        let bounds = handler.selection_bounds();
        assert_eq!(bounds, Some(((0, 0), (3, 3))));
    }

    #[test]
    fn test_mouse_handler_cell_size() {
        let mut handler = TerminalMouseHandler::new(10.0, 20.0);
        assert_eq!(handler.cell_size(), (10.0, 20.0));

        handler.set_cell_size(12.0, 24.0);
        assert_eq!(handler.cell_size(), (12.0, 24.0));
    }

    #[test]
    fn test_mouse_event_with_modifiers() {
        let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        let modifiers = TerminalModifiers {
            shift: true,
            control: true,
            alt: false,
        };
        let event =
            TerminalMouseEvent::with_modifiers(TerminalMouseButton::Left, 0, 0, true, modifiers);
        let bytes = event.to_bytes(mode);
        // Button 0 + shift(4) + ctrl(16) = 20
        assert_eq!(bytes, Some(b"\x1b[<20;1;1M".to_vec()));
    }
}
