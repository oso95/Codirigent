//! UI actions for Codirigent.
//!
//! This module defines the actions that can be triggered by keyboard
//! shortcuts or other input methods.
//!
//! # Action Types
//!
//! Actions are grouped by functionality:
//! - Layout actions: Change grid layout, toggle sidebar
//! - Focus actions: Navigate between sessions
//! - Session actions: Create, close, rename sessions
//!
//! # Example
//!
//! ```
//! use codirigent_ui::actions::{NextLayout, FocusSession, ToggleSidebar};
//!
//! let action = NextLayout;
//! let focus = FocusSession { number: 1 };
//! let toggle = ToggleSidebar;
//! ```

use crate::layout::FocusDirection;

/// Quit the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quit;

/// Cycle to the next layout profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NextLayout;

/// Cycle to the previous layout profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviousLayout;

/// Toggle sidebar visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleSidebar;

/// Focus a specific session by number (1-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusSession {
    /// Session number (1-based).
    pub number: usize,
}

impl FocusSession {
    /// Create a focus action for session 1.
    pub const fn session1() -> Self {
        Self { number: 1 }
    }

    /// Create a focus action for session 2.
    pub const fn session2() -> Self {
        Self { number: 2 }
    }

    /// Create a focus action for session 3.
    pub const fn session3() -> Self {
        Self { number: 3 }
    }

    /// Create a focus action for session 4.
    pub const fn session4() -> Self {
        Self { number: 4 }
    }

    /// Create a focus action for session 5.
    pub const fn session5() -> Self {
        Self { number: 5 }
    }

    /// Create a focus action for session 6.
    pub const fn session6() -> Self {
        Self { number: 6 }
    }

    /// Create a focus action for session 7.
    pub const fn session7() -> Self {
        Self { number: 7 }
    }

    /// Create a focus action for session 8.
    pub const fn session8() -> Self {
        Self { number: 8 }
    }

    /// Create a focus action for session 9.
    pub const fn session9() -> Self {
        Self { number: 9 }
    }
}

/// Focus the next session in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusNextSession;

/// Focus the previous session in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusPreviousSession;

/// Focus session in a direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusDirection_ {
    /// Direction to focus.
    pub direction: FocusDirection,
}

impl FocusDirection_ {
    /// Focus up.
    pub const fn up() -> Self {
        Self {
            direction: FocusDirection::Up,
        }
    }

    /// Focus down.
    pub const fn down() -> Self {
        Self {
            direction: FocusDirection::Down,
        }
    }

    /// Focus left.
    pub const fn left() -> Self {
        Self {
            direction: FocusDirection::Left,
        }
    }

    /// Focus right.
    pub const fn right() -> Self {
        Self {
            direction: FocusDirection::Right,
        }
    }
}

/// Create a new session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSession {
    /// Optional name for the session.
    pub name: Option<String>,
}

impl CreateSession {
    /// Create action with auto-generated name.
    pub fn new() -> Self {
        Self { name: None }
    }

    /// Create action with a specific name.
    pub fn with_name(name: String) -> Self {
        Self { name: Some(name) }
    }
}

impl Default for CreateSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Close the currently focused session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloseSession;

/// Close a specific session by ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloseSessionById {
    /// Session ID value.
    pub id: u64,
}

/// Rename the currently focused session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameSession {
    /// New name for the session.
    pub name: String,
}

/// Send input to the focused session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendInput {
    /// Input data to send.
    pub data: Vec<u8>,
}

impl SendInput {
    /// Create from a string.
    pub fn from_text(s: &str) -> Self {
        Self {
            data: s.as_bytes().to_vec(),
        }
    }

    /// Create from bytes.
    pub fn from_bytes(data: Vec<u8>) -> Self {
        Self { data }
    }
}

/// Toggle maximized state for focused session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleMaximize;

/// Set layout to a specific profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetLayout {
    /// Layout index (0-4).
    pub index: usize,
}

impl SetLayout {
    /// Set to 2x2 grid.
    pub const fn grid_2x2() -> Self {
        Self { index: 0 }
    }

    /// Set to 1x4 stack.
    pub const fn stack_1x4() -> Self {
        Self { index: 1 }
    }

    /// Set to 2x3 grid.
    pub const fn grid_2x3() -> Self {
        Self { index: 2 }
    }

    /// Set to 3x3 grid.
    pub const fn grid_3x3() -> Self {
        Self { index: 3 }
    }

    /// Set to single view.
    pub const fn single() -> Self {
        Self { index: 4 }
    }
}

/// Open settings/preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenSettings;

/// Split the focused pane horizontally (left-to-right).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitHorizontal;

/// Split the focused pane vertically (top-to-bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitVertical;

/// Close the focused pane (unsplit), promoting its sibling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosePane;

/// Open command palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenCommandPalette;

/// Reload configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReloadConfig;

/// Action handler trait for processing actions.
///
/// This trait allows workspace or other components to handle
/// actions in a type-safe way.
pub trait ActionHandler {
    /// Handle a quit action.
    fn handle_quit(&mut self) -> bool;
    /// Handle layout cycling.
    fn handle_next_layout(&mut self);
    /// Handle layout cycling backwards.
    fn handle_previous_layout(&mut self);
    /// Handle sidebar toggle.
    fn handle_toggle_sidebar(&mut self);
    /// Handle focusing a session.
    fn handle_focus_session(&mut self, number: usize) -> bool;
    /// Handle focusing next session.
    fn handle_focus_next(&mut self);
    /// Handle focusing previous session.
    fn handle_focus_previous(&mut self);
    /// Handle directional focus.
    fn handle_focus_direction(&mut self, direction: FocusDirection);
}

/// Keybinding configuration.
#[derive(Debug, Clone)]
pub struct KeyBinding {
    /// Key identifier (e.g., "Ctrl+L", "Alt+1").
    pub key: String,
    /// Action name.
    pub action: String,
}

impl KeyBinding {
    /// Create a new keybinding.
    pub fn new(key: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            action: action.into(),
        }
    }
}

/// Default keybindings for Codirigent.
pub fn default_keybindings() -> Vec<KeyBinding> {
    vec![
        // Layout
        KeyBinding::new("Ctrl+L", "next_layout"),
        KeyBinding::new("Ctrl+Shift+L", "previous_layout"),
        KeyBinding::new("Ctrl+B", "toggle_sidebar"),
        // Focus by number
        KeyBinding::new("Alt+1", "focus_session_1"),
        KeyBinding::new("Alt+2", "focus_session_2"),
        KeyBinding::new("Alt+3", "focus_session_3"),
        KeyBinding::new("Alt+4", "focus_session_4"),
        KeyBinding::new("Alt+5", "focus_session_5"),
        KeyBinding::new("Alt+6", "focus_session_6"),
        KeyBinding::new("Alt+7", "focus_session_7"),
        KeyBinding::new("Alt+8", "focus_session_8"),
        KeyBinding::new("Alt+9", "focus_session_9"),
        // Focus navigation
        KeyBinding::new("Ctrl+Tab", "focus_next"),
        KeyBinding::new("Ctrl+Shift+Tab", "focus_previous"),
        KeyBinding::new("Ctrl+Up", "focus_up"),
        KeyBinding::new("Ctrl+Down", "focus_down"),
        KeyBinding::new("Ctrl+Left", "focus_left"),
        KeyBinding::new("Ctrl+Right", "focus_right"),
        // Session management
        KeyBinding::new("Ctrl+N", "create_session"),
        KeyBinding::new("Ctrl+W", "close_session"),
        // Pane splitting
        KeyBinding::new("Ctrl+D", "split_horizontal"),
        KeyBinding::new("Ctrl+Shift+D", "split_vertical"),
        KeyBinding::new("Ctrl+Shift+W", "close_pane"),
        // Application
        KeyBinding::new("Ctrl+Q", "quit"),
        KeyBinding::new("Ctrl+,", "open_settings"),
        KeyBinding::new("Ctrl+Shift+P", "command_palette"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quit_action() {
        let action = Quit;
        assert_eq!(action, Quit);
    }

    #[test]
    fn test_layout_actions() {
        let next = NextLayout;
        let prev = PreviousLayout;
        assert_eq!(next, NextLayout);
        assert_eq!(prev, PreviousLayout);
    }

    #[test]
    fn test_toggle_sidebar() {
        let action = ToggleSidebar;
        assert_eq!(action, ToggleSidebar);
    }

    #[test]
    fn test_focus_session() {
        let f1 = FocusSession::session1();
        assert_eq!(f1.number, 1);

        let f9 = FocusSession::session9();
        assert_eq!(f9.number, 9);

        let f5 = FocusSession { number: 5 };
        assert_eq!(f5.number, 5);
    }

    #[test]
    fn test_focus_next_previous() {
        let next = FocusNextSession;
        let prev = FocusPreviousSession;
        assert_eq!(next, FocusNextSession);
        assert_eq!(prev, FocusPreviousSession);
    }

    #[test]
    fn test_focus_direction() {
        let up = FocusDirection_::up();
        assert_eq!(up.direction, FocusDirection::Up);

        let down = FocusDirection_::down();
        assert_eq!(down.direction, FocusDirection::Down);

        let left = FocusDirection_::left();
        assert_eq!(left.direction, FocusDirection::Left);

        let right = FocusDirection_::right();
        assert_eq!(right.direction, FocusDirection::Right);
    }

    #[test]
    fn test_create_session() {
        let action = CreateSession::new();
        assert!(action.name.is_none());

        let action = CreateSession::with_name("Test".to_string());
        assert_eq!(action.name, Some("Test".to_string()));

        let action = CreateSession::default();
        assert!(action.name.is_none());
    }

    #[test]
    fn test_close_session() {
        let close = CloseSession;
        assert_eq!(close, CloseSession);

        let close_by_id = CloseSessionById { id: 42 };
        assert_eq!(close_by_id.id, 42);
    }

    #[test]
    fn test_rename_session() {
        let action = RenameSession {
            name: "New Name".to_string(),
        };
        assert_eq!(action.name, "New Name");
    }

    #[test]
    fn test_send_input() {
        let action = SendInput::from_text("hello");
        assert_eq!(action.data, b"hello");

        let action = SendInput::from_bytes(vec![0x1b, 0x5b, 0x41]);
        assert_eq!(action.data, vec![0x1b, 0x5b, 0x41]);
    }

    #[test]
    fn test_toggle_maximize() {
        let action = ToggleMaximize;
        assert_eq!(action, ToggleMaximize);
    }

    #[test]
    fn test_set_layout() {
        assert_eq!(SetLayout::grid_2x2().index, 0);
        assert_eq!(SetLayout::stack_1x4().index, 1);
        assert_eq!(SetLayout::grid_2x3().index, 2);
        assert_eq!(SetLayout::grid_3x3().index, 3);
        assert_eq!(SetLayout::single().index, 4);
    }

    #[test]
    fn test_split_pane_actions() {
        let h = SplitHorizontal;
        let v = SplitVertical;
        let c = ClosePane;
        assert_eq!(h, SplitHorizontal);
        assert_eq!(v, SplitVertical);
        assert_eq!(c, ClosePane);
    }

    #[test]
    fn test_other_actions() {
        let settings = OpenSettings;
        let palette = OpenCommandPalette;
        let reload = ReloadConfig;

        assert_eq!(settings, OpenSettings);
        assert_eq!(palette, OpenCommandPalette);
        assert_eq!(reload, ReloadConfig);
    }

    #[test]
    fn test_keybinding_new() {
        let kb = KeyBinding::new("Ctrl+L", "next_layout");
        assert_eq!(kb.key, "Ctrl+L");
        assert_eq!(kb.action, "next_layout");
    }

    #[test]
    fn test_default_keybindings() {
        let bindings = default_keybindings();

        // Should have layout bindings
        assert!(bindings.iter().any(|b| b.action == "next_layout"));
        assert!(bindings.iter().any(|b| b.action == "toggle_sidebar"));

        // Should have session focus bindings
        assert!(bindings.iter().any(|b| b.action == "focus_session_1"));
        assert!(bindings.iter().any(|b| b.action == "focus_session_9"));

        // Should have navigation bindings
        assert!(bindings.iter().any(|b| b.action == "focus_next"));
        assert!(bindings.iter().any(|b| b.action == "focus_up"));

        // Should have pane splitting bindings
        assert!(bindings.iter().any(|b| b.action == "split_horizontal"));
        assert!(bindings.iter().any(|b| b.action == "split_vertical"));
        assert!(bindings.iter().any(|b| b.action == "close_pane"));

        // Should have app bindings
        assert!(bindings.iter().any(|b| b.action == "quit"));
        assert!(bindings.iter().any(|b| b.action == "create_session"));
    }

    #[test]
    fn test_keybinding_clone() {
        let kb = KeyBinding::new("Ctrl+Q", "quit");
        let cloned = kb.clone();
        assert_eq!(kb.key, cloned.key);
        assert_eq!(kb.action, cloned.action);
    }

    // Test for ActionHandler trait existence
    struct MockHandler;

    impl ActionHandler for MockHandler {
        fn handle_quit(&mut self) -> bool {
            true
        }
        fn handle_next_layout(&mut self) {}
        fn handle_previous_layout(&mut self) {}
        fn handle_toggle_sidebar(&mut self) {}
        fn handle_focus_session(&mut self, _: usize) -> bool {
            true
        }
        fn handle_focus_next(&mut self) {}
        fn handle_focus_previous(&mut self) {}
        fn handle_focus_direction(&mut self, _: FocusDirection) {}
    }

    #[test]
    fn test_action_handler_trait() {
        let mut handler = MockHandler;
        assert!(handler.handle_quit());
        handler.handle_next_layout();
        handler.handle_toggle_sidebar();
        assert!(handler.handle_focus_session(1));
        handler.handle_focus_next();
        handler.handle_focus_previous();
        handler.handle_focus_direction(FocusDirection::Up);
    }
}
