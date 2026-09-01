//! Keyboard input translation for terminal.
//!
//! This module provides functions for translating keyboard events into
//! terminal escape sequences.

use alacritty_terminal::term::TermMode;

/// Modifiers for terminal input.
///
/// Tracks which modifier keys are held during input events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerminalModifiers {
    /// Shift key is held.
    pub shift: bool,
    /// Control key is held.
    pub control: bool,
    /// Alt/Option key is held.
    pub alt: bool,
}

impl TerminalModifiers {
    /// Create new modifiers with all flags set to false.
    pub fn none() -> Self {
        Self::default()
    }

    /// Create modifiers with control flag set.
    pub fn ctrl() -> Self {
        Self {
            control: true,
            ..Self::default()
        }
    }

    /// Create modifiers with alt flag set.
    pub fn alt() -> Self {
        Self {
            alt: true,
            ..Self::default()
        }
    }

    /// Create modifiers with shift flag set.
    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Self::default()
        }
    }

    /// Calculate the modifier code for escape sequences.
    ///
    /// Returns the modifier code used in CSI sequences (1 + flags).
    /// - Shift = 1
    /// - Alt = 2
    /// - Ctrl = 4
    pub fn code(&self) -> u8 {
        let mut code = 0u8;
        if self.shift {
            code |= 1;
        }
        if self.alt {
            code |= 2;
        }
        if self.control {
            code |= 4;
        }
        if code > 0 {
            code + 1
        } else {
            0
        }
    }

    /// Check if any modifier is pressed.
    pub fn any(&self) -> bool {
        self.shift || self.control || self.alt
    }
}

/// A keystroke with key name and modifiers.
///
/// This is a simplified representation of a key event that can be
/// converted to terminal escape sequences.
#[derive(Debug, Clone)]
pub struct TerminalKeystroke {
    /// The key name (e.g., "a", "up", "f1", "enter").
    pub key: String,
    /// Active modifiers.
    pub modifiers: TerminalModifiers,
    /// IME composed key (for international input).
    pub ime_key: Option<String>,
}

impl TerminalKeystroke {
    /// Create a new keystroke.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            modifiers: TerminalModifiers::default(),
            ime_key: None,
        }
    }

    /// Create a keystroke with modifiers.
    pub fn with_modifiers(key: impl Into<String>, modifiers: TerminalModifiers) -> Self {
        Self {
            key: key.into(),
            modifiers,
            ime_key: None,
        }
    }

    /// Create a keystroke with IME key.
    pub fn with_ime(key: impl Into<String>, ime_key: String) -> Self {
        Self {
            key: key.into(),
            modifiers: TerminalModifiers::default(),
            ime_key: Some(ime_key),
        }
    }
}

/// Convert a keystroke to terminal input bytes.
///
/// This function translates key events into the appropriate escape sequences
/// for the terminal, respecting the current terminal mode (application cursor,
/// application keypad, etc.).
///
/// # Arguments
///
/// * `key` - The keystroke to convert
/// * `mode` - The current terminal mode flags
///
/// # Returns
///
/// The bytes to send to the PTY, or `None` if the key should not be sent.
pub fn key_to_bytes(key: &TerminalKeystroke, mode: TermMode) -> Option<Vec<u8>> {
    // If IME provided a composed character, use that
    if let Some(ref ime_key) = key.ime_key {
        return Some(ime_key.as_bytes().to_vec());
    }

    let app_cursor = mode.contains(TermMode::APP_CURSOR);
    let app_keypad = mode.contains(TermMode::APP_KEYPAD);
    let modifier_code = key.modifiers.code();

    // Handle special keys first
    match key.key.to_lowercase().as_str() {
        // Arrow keys
        "up" | "arrowup" => Some(arrow_key_bytes(b'A', app_cursor, modifier_code)),
        "down" | "arrowdown" => Some(arrow_key_bytes(b'B', app_cursor, modifier_code)),
        "right" | "arrowright" => Some(arrow_key_bytes(b'C', app_cursor, modifier_code)),
        "left" | "arrowleft" => Some(arrow_key_bytes(b'D', app_cursor, modifier_code)),

        // Function keys F1-F4 (special format)
        "f1" => Some(function_key_bytes(b'P', modifier_code)),
        "f2" => Some(function_key_bytes(b'Q', modifier_code)),
        "f3" => Some(function_key_bytes(b'R', modifier_code)),
        "f4" => Some(function_key_bytes(b'S', modifier_code)),

        // Function keys F5-F12 (CSI format)
        "f5" => Some(csi_tilde_bytes(15, modifier_code)),
        "f6" => Some(csi_tilde_bytes(17, modifier_code)),
        "f7" => Some(csi_tilde_bytes(18, modifier_code)),
        "f8" => Some(csi_tilde_bytes(19, modifier_code)),
        "f9" => Some(csi_tilde_bytes(20, modifier_code)),
        "f10" => Some(csi_tilde_bytes(21, modifier_code)),
        "f11" => Some(csi_tilde_bytes(23, modifier_code)),
        "f12" => Some(csi_tilde_bytes(24, modifier_code)),

        // Navigation keys
        "home" => Some(home_end_bytes(b'H', app_cursor, modifier_code)),
        "end" => Some(home_end_bytes(b'F', app_cursor, modifier_code)),
        "pageup" | "page_up" => Some(csi_tilde_bytes(5, modifier_code)),
        "pagedown" | "page_down" => Some(csi_tilde_bytes(6, modifier_code)),
        "insert" => Some(csi_tilde_bytes(2, modifier_code)),
        "delete" => Some(csi_tilde_bytes(3, modifier_code)),

        // Special keys
        "backspace" => handle_backspace(key),
        "tab" => handle_tab(key),
        "enter" | "return" => handle_enter(key),
        "escape" | "esc" => Some(vec![0x1b]),
        "space" | " " => handle_space(key),

        // Keypad numbers (when app_keypad mode)
        "numpad0" | "kp0" if app_keypad => Some(b"\x1bOp".to_vec()),
        "numpad1" | "kp1" if app_keypad => Some(b"\x1bOq".to_vec()),
        "numpad2" | "kp2" if app_keypad => Some(b"\x1bOr".to_vec()),
        "numpad3" | "kp3" if app_keypad => Some(b"\x1bOs".to_vec()),
        "numpad4" | "kp4" if app_keypad => Some(b"\x1bOt".to_vec()),
        "numpad5" | "kp5" if app_keypad => Some(b"\x1bOu".to_vec()),
        "numpad6" | "kp6" if app_keypad => Some(b"\x1bOv".to_vec()),
        "numpad7" | "kp7" if app_keypad => Some(b"\x1bOw".to_vec()),
        "numpad8" | "kp8" if app_keypad => Some(b"\x1bOx".to_vec()),
        "numpad9" | "kp9" if app_keypad => Some(b"\x1bOy".to_vec()),

        // Regular character input
        _ => handle_character_input(key),
    }
}

/// Generate bytes for arrow keys.
fn arrow_key_bytes(direction: u8, app_cursor: bool, modifier_code: u8) -> Vec<u8> {
    if modifier_code > 0 {
        format!("\x1b[1;{}{}", modifier_code, direction as char).into_bytes()
    } else if app_cursor {
        vec![0x1b, b'O', direction]
    } else {
        vec![0x1b, b'[', direction]
    }
}

/// Generate bytes for Home/End keys.
fn home_end_bytes(key: u8, app_cursor: bool, modifier_code: u8) -> Vec<u8> {
    if modifier_code > 0 {
        format!("\x1b[1;{}{}", modifier_code, key as char).into_bytes()
    } else if app_cursor {
        vec![0x1b, b'O', key]
    } else {
        vec![0x1b, b'[', key]
    }
}

/// Generate bytes for function keys F1-F4.
fn function_key_bytes(key: u8, modifier_code: u8) -> Vec<u8> {
    if modifier_code > 0 {
        format!("\x1b[1;{}{}", modifier_code, key as char).into_bytes()
    } else {
        vec![0x1b, b'O', key]
    }
}

/// Generate bytes for CSI ~ format keys.
fn csi_tilde_bytes(code: u8, modifier_code: u8) -> Vec<u8> {
    if modifier_code > 0 {
        format!("\x1b[{};{}~", code, modifier_code).into_bytes()
    } else {
        format!("\x1b[{}~", code).into_bytes()
    }
}

/// Handle backspace key.
fn handle_backspace(key: &TerminalKeystroke) -> Option<Vec<u8>> {
    if key.modifiers.control {
        Some(vec![0x08])
    } else if key.modifiers.alt {
        Some(vec![0x1b, 0x7f])
    } else {
        Some(vec![0x7f])
    }
}

/// Handle tab key.
fn handle_tab(key: &TerminalKeystroke) -> Option<Vec<u8>> {
    if key.modifiers.shift {
        Some(b"\x1b[Z".to_vec())
    } else {
        Some(vec![0x09])
    }
}

/// Handle enter key.
fn handle_enter(key: &TerminalKeystroke) -> Option<Vec<u8>> {
    if key.modifiers.alt {
        Some(vec![0x1b, 0x0d])
    } else {
        Some(vec![0x0d])
    }
}

/// Handle space key.
fn handle_space(key: &TerminalKeystroke) -> Option<Vec<u8>> {
    if key.modifiers.control {
        Some(vec![0x00])
    } else if key.modifiers.alt {
        Some(vec![0x1b, b' '])
    } else {
        Some(vec![b' '])
    }
}

/// Handle regular character input with modifiers.
fn handle_character_input(key: &TerminalKeystroke) -> Option<Vec<u8>> {
    if key.key.len() == 1 {
        let ch = key.key.chars().next()?;

        if key.modifiers.control {
            return handle_ctrl_char(ch);
        }

        if key.modifiers.alt {
            let mut bytes = vec![0x1b];
            if key.modifiers.shift {
                bytes.extend(ch.to_ascii_uppercase().to_string().as_bytes());
            } else {
                bytes.extend(ch.to_string().as_bytes());
            }
            return Some(bytes);
        }

        if key.modifiers.shift {
            Some(ch.to_ascii_uppercase().to_string().into_bytes())
        } else {
            Some(ch.to_string().into_bytes())
        }
    } else {
        None
    }
}

/// Handle Ctrl+character combinations.
fn handle_ctrl_char(ch: char) -> Option<Vec<u8>> {
    match ch.to_ascii_lowercase() {
        'a'..='z' => {
            let ctrl_char = ch.to_ascii_lowercase() as u8 - b'a' + 1;
            Some(vec![ctrl_char])
        }
        '@' => Some(vec![0x00]),
        '[' => Some(vec![0x1b]),
        '\\' => Some(vec![0x1c]),
        ']' => Some(vec![0x1d]),
        '^' => Some(vec![0x1e]),
        '_' => Some(vec![0x1f]),
        '?' => Some(vec![0x7f]),
        '2' => Some(vec![0x00]),
        '3' => Some(vec![0x1b]),
        '4' => Some(vec![0x1c]),
        '5' => Some(vec![0x1d]),
        '6' => Some(vec![0x1e]),
        '7' => Some(vec![0x1f]),
        '8' => Some(vec![0x7f]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_keystroke(key: &str, ctrl: bool, alt: bool, shift: bool) -> TerminalKeystroke {
        TerminalKeystroke {
            key: key.to_string(),
            modifiers: TerminalModifiers {
                control: ctrl,
                alt,
                shift,
            },
            ime_key: None,
        }
    }

    #[test]
    fn test_arrow_keys_normal_mode() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("up", false, false, false), mode),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("down", false, false, false), mode),
            Some(b"\x1b[B".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("right", false, false, false), mode),
            Some(b"\x1b[C".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("left", false, false, false), mode),
            Some(b"\x1b[D".to_vec())
        );
    }

    #[test]
    fn test_arrow_keys_app_cursor_mode() {
        let mode = TermMode::APP_CURSOR;
        assert_eq!(
            key_to_bytes(&make_keystroke("up", false, false, false), mode),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("down", false, false, false), mode),
            Some(b"\x1bOB".to_vec())
        );
    }

    #[test]
    fn test_arrow_keys_with_modifiers() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("up", true, false, false), mode),
            Some(b"\x1b[1;5A".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("up", false, true, false), mode),
            Some(b"\x1b[1;3A".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("up", false, false, true), mode),
            Some(b"\x1b[1;2A".to_vec())
        );
    }

    #[test]
    fn test_ctrl_c() {
        let mode = TermMode::empty();
        let bytes = key_to_bytes(&make_keystroke("c", true, false, false), mode);
        assert_eq!(bytes, Some(vec![3]));
    }

    #[test]
    fn test_ctrl_d() {
        let mode = TermMode::empty();
        let bytes = key_to_bytes(&make_keystroke("d", true, false, false), mode);
        assert_eq!(bytes, Some(vec![4]));
    }

    #[test]
    fn test_ctrl_z() {
        let mode = TermMode::empty();
        let bytes = key_to_bytes(&make_keystroke("z", true, false, false), mode);
        assert_eq!(bytes, Some(vec![26]));
    }

    #[test]
    fn test_regular_char() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("a", false, false, false), mode),
            Some(b"a".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("z", false, false, false), mode),
            Some(b"z".to_vec())
        );
    }

    #[test]
    fn test_regular_char_with_shift() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("a", false, false, true), mode),
            Some(b"A".to_vec())
        );
    }

    #[test]
    fn test_alt_char() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("a", false, true, false), mode),
            Some(vec![0x1b, b'a'])
        );
    }

    #[test]
    fn test_function_keys() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("f1", false, false, false), mode),
            Some(b"\x1bOP".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("f5", false, false, false), mode),
            Some(b"\x1b[15~".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("f12", false, false, false), mode),
            Some(b"\x1b[24~".to_vec())
        );
    }

    #[test]
    fn test_navigation_keys() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("home", false, false, false), mode),
            Some(b"\x1b[H".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("end", false, false, false), mode),
            Some(b"\x1b[F".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("pageup", false, false, false), mode),
            Some(b"\x1b[5~".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("delete", false, false, false), mode),
            Some(b"\x1b[3~".to_vec())
        );
    }

    #[test]
    fn test_special_keys() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("backspace", false, false, false), mode),
            Some(vec![0x7f])
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("tab", false, false, false), mode),
            Some(vec![0x09])
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("enter", false, false, false), mode),
            Some(vec![0x0d])
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("escape", false, false, false), mode),
            Some(vec![0x1b])
        );
    }

    #[test]
    fn test_terminal_editing_control_sequences() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("backspace", true, false, false), mode),
            Some(vec![0x08])
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("w", true, false, false), mode),
            Some(vec![0x17])
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("left", true, false, false), mode),
            Some(b"\x1b[1;5D".to_vec())
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("delete", true, false, false), mode),
            Some(b"\x1b[3;5~".to_vec())
        );
    }

    #[test]
    fn test_shift_tab() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("tab", false, false, true), mode),
            Some(b"\x1b[Z".to_vec())
        );
    }

    #[test]
    fn test_ctrl_special_chars() {
        let mode = TermMode::empty();
        assert_eq!(
            key_to_bytes(&make_keystroke("[", true, false, false), mode),
            Some(vec![0x1b])
        );
        assert_eq!(
            key_to_bytes(&make_keystroke("@", true, false, false), mode),
            Some(vec![0x00])
        );
    }

    #[test]
    fn test_modifier_code() {
        assert_eq!(TerminalModifiers::none().code(), 0);
        assert_eq!(TerminalModifiers::shift().code(), 2);
        assert_eq!(TerminalModifiers::alt().code(), 3);
        assert_eq!(TerminalModifiers::ctrl().code(), 5);

        let all = TerminalModifiers {
            shift: true,
            alt: true,
            control: true,
        };
        assert_eq!(all.code(), 8);
    }

    #[test]
    fn test_ime_key() {
        let mode = TermMode::empty();
        let key = TerminalKeystroke::with_ime("", "\u{4E2D}".to_string());
        let bytes = key_to_bytes(&key, mode);
        assert_eq!(bytes, Some("\u{4E2D}".as_bytes().to_vec()));
    }
}
