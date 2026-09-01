//! Keybinding system for Codirigent.
//!
//! This module provides a flexible keybinding manager that supports:
//! - Configurable keyboard shortcuts
//! - Modifier key combinations (Ctrl, Alt, Shift, Cmd)
//! - Parsing keybinding strings (e.g., "Cmd+Shift+N")
//! - Reverse lookup of bindings for actions
//!
//! # Example
//!
//! ```
//! use codirigent_ui::keybindings::{KeybindingManager, KeyBinding, Modifiers, Action};
//!
//! let manager = KeybindingManager::with_defaults();
//! // Platform modifier: Cmd on macOS, Ctrl elsewhere.
//! #[cfg(target_os = "macos")]
//! let binding = KeybindingManager::parse_binding("Cmd+Shift+N").unwrap();
//! #[cfg(not(target_os = "macos"))]
//! let binding = KeybindingManager::parse_binding("Ctrl+Shift+N").unwrap();
//! assert_eq!(manager.get_action(&binding), Some(&Action::NewSession));
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Keyboard modifier flags.
///
/// Represents which modifier keys are pressed in a key combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Modifiers {
    /// Control key (Ctrl).
    pub ctrl: bool,
    /// Alt/Option key.
    pub alt: bool,
    /// Shift key.
    pub shift: bool,
    /// Command/Meta/Super key.
    pub cmd: bool,
}

impl Modifiers {
    /// Create modifiers with no keys pressed.
    pub const fn none() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
        }
    }

    /// Create modifiers with only Ctrl pressed.
    pub const fn ctrl() -> Self {
        Self {
            ctrl: true,
            alt: false,
            shift: false,
            cmd: false,
        }
    }

    /// Create modifiers with only Cmd pressed.
    pub const fn cmd() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            cmd: true,
        }
    }

    /// Create modifiers with only Alt pressed.
    pub const fn alt() -> Self {
        Self {
            ctrl: false,
            alt: true,
            shift: false,
            cmd: false,
        }
    }

    /// Create modifiers with only Shift pressed.
    pub const fn shift() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: true,
            cmd: false,
        }
    }

    /// Check if any modifier is pressed.
    pub const fn any(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.cmd
    }

    /// Check if no modifiers are pressed.
    pub const fn is_empty(&self) -> bool {
        !self.any()
    }
}

/// A key combination (key + modifiers).
///
/// Represents a specific keyboard shortcut that can be bound to an action.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyBinding {
    /// The key (e.g., "N", "1", "Tab", "Enter").
    pub key: String,
    /// Modifier keys.
    pub modifiers: Modifiers,
}

impl KeyBinding {
    /// Create a new key binding.
    pub fn new(key: impl Into<String>, modifiers: Modifiers) -> Self {
        Self {
            key: key.into(),
            modifiers,
        }
    }

    /// Create a binding with no modifiers.
    pub fn bare(key: impl Into<String>) -> Self {
        Self::new(key, Modifiers::none())
    }

    /// Create a binding with Ctrl modifier.
    pub fn with_ctrl(key: impl Into<String>) -> Self {
        Self::new(key, Modifiers::ctrl())
    }

    /// Create a binding with Cmd modifier.
    pub fn with_cmd(key: impl Into<String>) -> Self {
        Self::new(key, Modifiers::cmd())
    }

    /// Create a binding with Alt modifier.
    pub fn with_alt(key: impl Into<String>) -> Self {
        Self::new(key, Modifiers::alt())
    }
}

/// Action that can be bound to a key.
///
/// Represents all the actions that can be triggered via keyboard shortcuts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    // Session actions
    /// Create a new session.
    NewSession,
    /// Close the current session.
    CloseSession,
    /// Switch to session by number (1-based).
    SwitchSession(usize),
    /// Focus the next session.
    NextSession,
    /// Focus the previous session.
    PreviousSession,
    /// Focus session by number (1-based).
    FocusSession(usize),

    // Layout actions
    /// Toggle between layouts.
    ToggleLayout,
    /// Set a specific layout by name.
    SetLayout(String),
    /// Toggle sidebar visibility.
    ToggleSidebar,
    /// Toggle task board visibility.
    ToggleTaskBoard,

    // Input actions
    /// Send a predefined input string.
    SendInput(String),
    /// Toggle broadcast mode.
    Broadcast,

    // Navigation
    /// Open quick switch dialog.
    QuickSwitch,
    /// Open command palette.
    CommandPalette,

    // Clipboard
    /// Copy selection.
    Copy,
    /// Paste from clipboard.
    Paste,
    /// Search inside the active terminal.
    SearchTerminal,
    /// Copy entire session output.
    CopySessionOutput,

    // Application
    /// Quit the application.
    Quit,
    /// Open settings.
    OpenSettings,
    /// Reload configuration.
    ReloadConfig,

    // Custom action (for plugins)
    /// Custom action identified by name.
    Custom(String),
}

/// Keybinding manager.
///
/// Manages keyboard shortcuts and their associated actions.
/// Supports default bindings, custom configuration, and reverse lookup.
#[derive(Debug, Clone)]
pub struct KeybindingManager {
    /// Forward mapping: binding -> action.
    bindings: HashMap<KeyBinding, Action>,
    /// Reverse mapping: action -> bindings.
    reverse_map: HashMap<Action, Vec<KeyBinding>>,
}

impl KeybindingManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            reverse_map: HashMap::new(),
        }
    }

    /// Create with default bindings.
    ///
    /// Sets up standard keybindings per the Codirigent spec.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::keybindings::{KeybindingManager, Action};
    ///
    /// let manager = KeybindingManager::with_defaults();
    /// // Platform modifier key: Cmd on macOS, Ctrl elsewhere.
    /// #[cfg(target_os = "macos")]
    /// let binding = KeybindingManager::parse_binding("Cmd+Shift+N").unwrap();
    /// #[cfg(not(target_os = "macos"))]
    /// let binding = KeybindingManager::parse_binding("Ctrl+Shift+N").unwrap();
    /// assert_eq!(manager.get_action(&binding), Some(&Action::NewSession));
    /// ```
    pub fn with_defaults() -> Self {
        // Use platform modifier: Cmd on macOS, Ctrl elsewhere.
        #[cfg(target_os = "macos")]
        let m = "Cmd";
        #[cfg(not(target_os = "macos"))]
        let m = "Ctrl";

        let mut manager = Self::new();

        // Session number shortcuts
        for i in 1..=9 {
            if let Ok(binding) = Self::parse_binding(&format!("{m}+{i}")) {
                manager.set_binding(binding, Action::SwitchSession(i));
            }
        }

        // Session management
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Shift+N")) {
            manager.set_binding(binding, Action::NewSession);
        }
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Alt+W")) {
            manager.set_binding(binding, Action::CloseSession);
        }

        // Navigation
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Shift+K")) {
            manager.set_binding(binding, Action::QuickSwitch);
        }
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Shift+P")) {
            manager.set_binding(binding, Action::CommandPalette);
        }
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Tab")) {
            manager.set_binding(binding, Action::NextSession);
        }
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Shift+Tab")) {
            manager.set_binding(binding, Action::PreviousSession);
        }

        // Layout
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Shift+L")) {
            manager.set_binding(binding, Action::ToggleLayout);
        }
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Shift+E")) {
            manager.set_binding(binding, Action::ToggleSidebar);
        }
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Shift+B")) {
            manager.set_binding(binding, Action::Broadcast);
        }
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Shift+T")) {
            manager.set_binding(binding, Action::ToggleTaskBoard);
        }
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Shift+F")) {
            manager.set_binding(binding, Action::SearchTerminal);
        }

        // Clipboard
        if let Ok(binding) = Self::parse_binding(&format!("{m}+C")) {
            manager.set_binding(binding, Action::Copy);
        }
        if let Ok(binding) = Self::parse_binding(&format!("{m}+V")) {
            manager.set_binding(binding, Action::Paste);
        }

        // Application
        if let Ok(binding) = Self::parse_binding(&format!("{m}+Q")) {
            manager.set_binding(binding, Action::Quit);
        }
        if let Ok(binding) = Self::parse_binding(&format!("{m}+,")) {
            manager.set_binding(binding, Action::OpenSettings);
        }

        manager
    }

    /// Load bindings from a config map.
    ///
    /// Config format: action_name -> binding_string
    ///
    /// # Arguments
    ///
    /// * `config` - Map of action names to binding strings
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::keybindings::KeybindingManager;
    /// use std::collections::HashMap;
    ///
    /// let mut config = HashMap::new();
    /// config.insert("new_session".to_string(), "Ctrl+Shift+N".to_string());
    /// let manager = KeybindingManager::from_config(&config);
    /// ```
    pub fn from_config(config: &HashMap<String, String>) -> Self {
        let mut manager = Self::with_defaults();
        for (action_name, binding_str) in config {
            match Self::parse_binding(binding_str) {
                Ok(binding) => match Self::action_from_name(action_name) {
                    Some(action) => {
                        manager.set_binding(binding, action);
                    }
                    None => {
                        tracing::warn!(
                            action = %action_name,
                            "Ignoring unknown keybinding action in user settings"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        action = %action_name,
                        binding = %binding_str,
                        error = %e,
                        "Ignoring unparseable keybinding in user settings"
                    );
                }
            }
        }
        manager
    }

    /// Get action for a key combination.
    ///
    /// # Arguments
    ///
    /// * `binding` - The key combination to look up
    ///
    /// # Returns
    ///
    /// The action bound to this key, or None if not bound.
    pub fn get_action(&self, binding: &KeyBinding) -> Option<&Action> {
        self.bindings.get(binding)
    }

    /// Get all bindings for an action.
    ///
    /// # Arguments
    ///
    /// * `action` - The action to look up
    ///
    /// # Returns
    ///
    /// Slice of all key bindings that trigger this action.
    pub fn get_bindings(&self, action: &Action) -> &[KeyBinding] {
        self.reverse_map.get(action).map_or(&[], |v| v.as_slice())
    }

    /// Set a binding (overwrites existing).
    ///
    /// # Arguments
    ///
    /// * `binding` - The key combination
    /// * `action` - The action to bind
    pub fn set_binding(&mut self, binding: KeyBinding, action: Action) {
        // Remove old reverse mapping if this binding existed
        if let Some(old_action) = self.bindings.get(&binding) {
            if let Some(bindings) = self.reverse_map.get_mut(old_action) {
                bindings.retain(|b| b != &binding);
            }
        }

        // Add new mapping
        self.bindings.insert(binding.clone(), action.clone());
        self.reverse_map.entry(action).or_default().push(binding);
    }

    /// Remove a binding.
    ///
    /// # Arguments
    ///
    /// * `binding` - The key combination to remove
    pub fn remove_binding(&mut self, binding: &KeyBinding) {
        if let Some(action) = self.bindings.remove(binding) {
            if let Some(bindings) = self.reverse_map.get_mut(&action) {
                bindings.retain(|b| b != binding);
            }
        }
    }

    /// List all bindings.
    ///
    /// # Returns
    ///
    /// Iterator over all (binding, action) pairs.
    pub fn list_bindings(&self) -> impl Iterator<Item = (&KeyBinding, &Action)> {
        self.bindings.iter()
    }

    /// Parse a binding string like "Cmd+Shift+N".
    ///
    /// Supported modifiers:
    /// - Cmd, Meta, Super - Command key
    /// - Ctrl, Control - Control key
    /// - Alt, Option - Alt/Option key
    /// - Shift - Shift key
    ///
    /// # Arguments
    ///
    /// * `s` - The binding string to parse
    ///
    /// # Returns
    ///
    /// The parsed KeyBinding or an error message.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::keybindings::KeybindingManager;
    ///
    /// let binding = KeybindingManager::parse_binding("Cmd+Shift+N").unwrap();
    /// assert!(binding.modifiers.cmd);
    /// assert!(binding.modifiers.shift);
    /// assert_eq!(binding.key, "N");
    /// ```
    pub fn parse_binding(s: &str) -> Result<KeyBinding, String> {
        let parts: Vec<&str> = s.split('+').collect();
        if parts.is_empty() {
            return Err("Empty binding".to_string());
        }

        let mut modifiers = Modifiers::default();
        let mut key = String::new();

        for (i, part) in parts.iter().enumerate() {
            let part = part.trim();
            if i == parts.len() - 1 {
                // Last part is the key
                key = part.to_string();
            } else {
                // Other parts are modifiers
                match part.to_lowercase().as_str() {
                    "cmd" | "meta" | "super" | "command" => modifiers.cmd = true,
                    "ctrl" | "control" => modifiers.ctrl = true,
                    "alt" | "option" => modifiers.alt = true,
                    "shift" => modifiers.shift = true,
                    _ => return Err(format!("Unknown modifier: {}", part)),
                }
            }
        }

        if key.is_empty() {
            return Err("No key specified".to_string());
        }

        Ok(KeyBinding { key, modifiers })
    }

    /// Format a binding for display.
    ///
    /// Creates a human-readable string representation of a key binding.
    ///
    /// # Arguments
    ///
    /// * `binding` - The key binding to format
    ///
    /// # Returns
    ///
    /// A string like "Cmd+Shift+N".
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::keybindings::{KeybindingManager, KeyBinding, Modifiers};
    ///
    /// let binding = KeyBinding::new("N", Modifiers { cmd: true, shift: true, ..Default::default() });
    /// let result = KeybindingManager::format_binding(&binding);
    /// #[cfg(target_os = "macos")]
    /// assert_eq!(result, "Cmd+Shift+N");
    /// #[cfg(not(target_os = "macos"))]
    /// assert_eq!(result, "Ctrl+Shift+N");
    /// ```
    pub fn format_binding(binding: &KeyBinding) -> String {
        let mut parts = Vec::new();
        if binding.modifiers.cmd {
            // On macOS show "Cmd"; on other platforms the platform modifier key
            // is Ctrl, so display it as "Ctrl" to avoid confusing users.
            #[cfg(target_os = "macos")]
            parts.push("Cmd");
            #[cfg(not(target_os = "macos"))]
            parts.push("Ctrl");
        }
        if binding.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if binding.modifiers.alt {
            parts.push("Alt");
        }
        if binding.modifiers.shift {
            parts.push("Shift");
        }
        parts.push(&binding.key);
        parts.join("+")
    }

    /// Convert action name to Action enum.
    fn action_from_name(name: &str) -> Option<Action> {
        match name {
            "new_session" => Some(Action::NewSession),
            "close_session" => Some(Action::CloseSession),
            "next_session" => Some(Action::NextSession),
            "previous_session" => Some(Action::PreviousSession),
            "quick_switch" => Some(Action::QuickSwitch),
            "command_palette" => Some(Action::CommandPalette),
            "toggle_layout" => Some(Action::ToggleLayout),
            "toggle_sidebar" => Some(Action::ToggleSidebar),
            "toggle_task_board" => Some(Action::ToggleTaskBoard),
            "broadcast" => Some(Action::Broadcast),
            "copy" => Some(Action::Copy),
            "paste" => Some(Action::Paste),
            "search_terminal" => Some(Action::SearchTerminal),
            "copy_session_output" => Some(Action::CopySessionOutput),
            "quit" => Some(Action::Quit),
            "open_settings" => Some(Action::OpenSettings),
            "reload_config" => Some(Action::ReloadConfig),
            s if s.starts_with("switch_session_") => s
                .strip_prefix("switch_session_")
                .and_then(|n| n.parse().ok())
                .map(Action::SwitchSession),
            s if s.starts_with("focus_session_") => s
                .strip_prefix("focus_session_")
                .and_then(|n| n.parse().ok())
                .map(Action::FocusSession),
            s if s.starts_with("custom:") => s
                .strip_prefix("custom:")
                .map(|n| Action::Custom(n.to_string())),
            s if s.starts_with("set_layout:") => s
                .strip_prefix("set_layout:")
                .map(|n| Action::SetLayout(n.to_string())),
            s if s.starts_with("send_input:") => s
                .strip_prefix("send_input:")
                .map(|n| Action::SendInput(n.to_string())),
            _ => None,
        }
    }

    /// Convert Action to action name string.
    pub fn action_to_name(action: &Action) -> String {
        match action {
            Action::NewSession => "new_session".to_string(),
            Action::CloseSession => "close_session".to_string(),
            Action::SwitchSession(n) => format!("switch_session_{}", n),
            Action::NextSession => "next_session".to_string(),
            Action::PreviousSession => "previous_session".to_string(),
            Action::FocusSession(n) => format!("focus_session_{}", n),
            Action::ToggleLayout => "toggle_layout".to_string(),
            Action::SetLayout(name) => format!("set_layout:{}", name),
            Action::ToggleSidebar => "toggle_sidebar".to_string(),
            Action::ToggleTaskBoard => "toggle_task_board".to_string(),
            Action::SendInput(input) => format!("send_input:{}", input),
            Action::Broadcast => "broadcast".to_string(),
            Action::QuickSwitch => "quick_switch".to_string(),
            Action::CommandPalette => "command_palette".to_string(),
            Action::Copy => "copy".to_string(),
            Action::Paste => "paste".to_string(),
            Action::SearchTerminal => "search_terminal".to_string(),
            Action::CopySessionOutput => "copy_session_output".to_string(),
            Action::Quit => "quit".to_string(),
            Action::OpenSettings => "open_settings".to_string(),
            Action::ReloadConfig => "reload_config".to_string(),
            Action::Custom(name) => format!("custom:{}", name),
        }
    }
}

impl Default for KeybindingManager {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifiers_none() {
        let m = Modifiers::none();
        assert!(!m.ctrl);
        assert!(!m.alt);
        assert!(!m.shift);
        assert!(!m.cmd);
        assert!(!m.any());
        assert!(m.is_empty());
    }

    #[test]
    fn test_modifiers_ctrl() {
        let m = Modifiers::ctrl();
        assert!(m.ctrl);
        assert!(!m.cmd);
        assert!(m.any());
        assert!(!m.is_empty());
    }

    #[test]
    fn test_modifiers_cmd() {
        let m = Modifiers::cmd();
        assert!(m.cmd);
        assert!(!m.ctrl);
    }

    #[test]
    fn test_modifiers_alt() {
        let m = Modifiers::alt();
        assert!(m.alt);
        assert!(!m.ctrl);
    }

    #[test]
    fn test_modifiers_shift() {
        let m = Modifiers::shift();
        assert!(m.shift);
        assert!(!m.ctrl);
    }

    #[test]
    fn test_modifiers_default() {
        let m = Modifiers::default();
        assert!(m.is_empty());
    }

    #[test]
    fn test_modifiers_serialization() {
        let m = Modifiers {
            cmd: true,
            shift: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&m).unwrap();
        let parsed: Modifiers = serde_json::from_str(&json).unwrap();
        assert_eq!(m, parsed);
    }

    #[test]
    fn test_keybinding_new() {
        let kb = KeyBinding::new("N", Modifiers::cmd());
        assert_eq!(kb.key, "N");
        assert!(kb.modifiers.cmd);
    }

    #[test]
    fn test_keybinding_bare() {
        let kb = KeyBinding::bare("Escape");
        assert_eq!(kb.key, "Escape");
        assert!(kb.modifiers.is_empty());
    }

    #[test]
    fn test_keybinding_with_ctrl() {
        let kb = KeyBinding::with_ctrl("C");
        assert!(kb.modifiers.ctrl);
    }

    #[test]
    fn test_keybinding_with_cmd() {
        let kb = KeyBinding::with_cmd("C");
        assert!(kb.modifiers.cmd);
    }

    #[test]
    fn test_keybinding_with_alt() {
        let kb = KeyBinding::with_alt("Tab");
        assert!(kb.modifiers.alt);
    }

    #[test]
    fn test_keybinding_serialization() {
        let kb = KeyBinding::new("N", Modifiers::cmd());
        let json = serde_json::to_string(&kb).unwrap();
        let parsed: KeyBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(kb, parsed);
    }

    #[test]
    fn test_parse_binding_simple() {
        let binding = KeybindingManager::parse_binding("N").unwrap();
        assert_eq!(binding.key, "N");
        assert!(binding.modifiers.is_empty());
    }

    #[test]
    fn test_parse_binding_with_cmd() {
        let binding = KeybindingManager::parse_binding("Cmd+N").unwrap();
        assert_eq!(binding.key, "N");
        assert!(binding.modifiers.cmd);
        assert!(!binding.modifiers.ctrl);
    }

    #[test]
    fn test_parse_binding_multiple_modifiers() {
        let binding = KeybindingManager::parse_binding("Cmd+Shift+N").unwrap();
        assert!(binding.modifiers.cmd);
        assert!(binding.modifiers.shift);
        assert_eq!(binding.key, "N");
    }

    #[test]
    fn test_parse_binding_all_modifiers() {
        let binding = KeybindingManager::parse_binding("Ctrl+Alt+Shift+Cmd+X").unwrap();
        assert!(binding.modifiers.ctrl);
        assert!(binding.modifiers.alt);
        assert!(binding.modifiers.shift);
        assert!(binding.modifiers.cmd);
        assert_eq!(binding.key, "X");
    }

    #[test]
    fn test_parse_binding_lowercase() {
        let binding = KeybindingManager::parse_binding("cmd+shift+n").unwrap();
        assert!(binding.modifiers.cmd);
        assert!(binding.modifiers.shift);
        assert_eq!(binding.key, "n");
    }

    #[test]
    fn test_parse_binding_alt_names() {
        let binding = KeybindingManager::parse_binding("Meta+Option+Control+a").unwrap();
        assert!(binding.modifiers.cmd); // Meta -> cmd
        assert!(binding.modifiers.alt); // Option -> alt
        assert!(binding.modifiers.ctrl); // Control -> ctrl
    }

    #[test]
    fn test_parse_binding_special_keys() {
        let binding = KeybindingManager::parse_binding("Cmd+Tab").unwrap();
        assert_eq!(binding.key, "Tab");

        let binding = KeybindingManager::parse_binding("Cmd+\\").unwrap();
        assert_eq!(binding.key, "\\");
    }

    #[test]
    fn test_parse_binding_error_empty() {
        let result = KeybindingManager::parse_binding("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_binding_error_unknown_modifier() {
        let result = KeybindingManager::parse_binding("Unknown+N");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown modifier"));
    }

    #[test]
    fn test_format_binding() {
        let binding = KeyBinding::new(
            "N",
            Modifiers {
                cmd: true,
                shift: true,
                ..Default::default()
            },
        );
        #[cfg(target_os = "macos")]
        assert_eq!(KeybindingManager::format_binding(&binding), "Cmd+Shift+N");
        #[cfg(not(target_os = "macos"))]
        assert_eq!(KeybindingManager::format_binding(&binding), "Ctrl+Shift+N");
    }

    #[test]
    fn test_format_binding_all_modifiers() {
        let binding = KeyBinding::new(
            "X",
            Modifiers {
                cmd: true,
                ctrl: true,
                alt: true,
                shift: true,
            },
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            KeybindingManager::format_binding(&binding),
            "Cmd+Ctrl+Alt+Shift+X"
        );
        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            KeybindingManager::format_binding(&binding),
            "Ctrl+Ctrl+Alt+Shift+X"
        );
    }

    #[test]
    fn test_format_binding_no_modifiers() {
        let binding = KeyBinding::bare("Escape");
        assert_eq!(KeybindingManager::format_binding(&binding), "Escape");
    }

    #[test]
    fn test_keybinding_manager_new() {
        let manager = KeybindingManager::new();
        assert!(manager.bindings.is_empty());
    }

    #[test]
    fn test_keybinding_manager_with_defaults() {
        let manager = KeybindingManager::with_defaults();

        // Check some default bindings (platform modifier: Cmd on macOS, Ctrl elsewhere)
        #[cfg(target_os = "macos")]
        let (new_key, close_key) = ("Cmd+Shift+N", "Cmd+Alt+W");
        #[cfg(not(target_os = "macos"))]
        let (new_key, close_key) = ("Ctrl+Shift+N", "Ctrl+Alt+W");

        let binding = KeybindingManager::parse_binding(new_key).unwrap();
        assert_eq!(manager.get_action(&binding), Some(&Action::NewSession));

        let binding = KeybindingManager::parse_binding(close_key).unwrap();
        assert_eq!(manager.get_action(&binding), Some(&Action::CloseSession));
    }

    #[test]
    fn test_default_bindings() {
        let manager = KeybindingManager::with_defaults();
        #[cfg(target_os = "macos")]
        let key = "Cmd+Shift+N";
        #[cfg(not(target_os = "macos"))]
        let key = "Ctrl+Shift+N";
        let binding = KeybindingManager::parse_binding(key).unwrap();
        assert_eq!(manager.get_action(&binding), Some(&Action::NewSession));
    }

    #[test]
    fn test_set_binding() {
        let mut manager = KeybindingManager::new();
        let binding = KeyBinding::with_cmd("X");
        manager.set_binding(binding.clone(), Action::Quit);
        assert_eq!(manager.get_action(&binding), Some(&Action::Quit));
    }

    #[test]
    fn test_set_binding_overwrites() {
        let mut manager = KeybindingManager::new();
        let binding = KeyBinding::with_cmd("X");
        manager.set_binding(binding.clone(), Action::Quit);
        manager.set_binding(binding.clone(), Action::Copy);
        assert_eq!(manager.get_action(&binding), Some(&Action::Copy));
    }

    #[test]
    fn test_remove_binding() {
        let mut manager = KeybindingManager::new();
        let binding = KeyBinding::with_cmd("X");
        manager.set_binding(binding.clone(), Action::Quit);
        manager.remove_binding(&binding);
        assert_eq!(manager.get_action(&binding), None);
    }

    #[test]
    fn test_get_bindings_for_action() {
        let manager = KeybindingManager::with_defaults();
        let bindings = manager.get_bindings(&Action::NewSession);
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_get_bindings_empty() {
        let manager = KeybindingManager::new();
        let bindings = manager.get_bindings(&Action::Quit);
        assert!(bindings.is_empty());
    }

    #[test]
    fn test_list_bindings() {
        let manager = KeybindingManager::with_defaults();
        let count = manager.list_bindings().count();
        assert!(count > 0);
    }

    #[test]
    fn test_from_config() {
        let mut config = HashMap::new();
        config.insert("new_session".to_string(), "Ctrl+Shift+N".to_string());
        let manager = KeybindingManager::from_config(&config);

        let binding = KeybindingManager::parse_binding("Ctrl+Shift+N").unwrap();
        assert_eq!(manager.get_action(&binding), Some(&Action::NewSession));
    }

    #[test]
    fn test_from_config_ignores_invalid() {
        let mut config = HashMap::new();
        config.insert("new_session".to_string(), "Invalid+Key+Combo".to_string());
        let manager = KeybindingManager::from_config(&config);
        // Should still have defaults (platform modifier: Cmd on macOS, Ctrl elsewhere)
        #[cfg(target_os = "macos")]
        let key = "Cmd+Shift+N";
        #[cfg(not(target_os = "macos"))]
        let key = "Ctrl+Shift+N";
        let binding = KeybindingManager::parse_binding(key).unwrap();
        assert_eq!(manager.get_action(&binding), Some(&Action::NewSession));
    }

    #[test]
    fn test_action_from_name() {
        assert_eq!(
            KeybindingManager::action_from_name("new_session"),
            Some(Action::NewSession)
        );
        assert_eq!(
            KeybindingManager::action_from_name("quit"),
            Some(Action::Quit)
        );
        assert_eq!(
            KeybindingManager::action_from_name("switch_session_5"),
            Some(Action::SwitchSession(5))
        );
        assert_eq!(
            KeybindingManager::action_from_name("custom:my_action"),
            Some(Action::Custom("my_action".to_string()))
        );
        assert_eq!(KeybindingManager::action_from_name("unknown"), None);
    }

    #[test]
    fn test_action_to_name() {
        assert_eq!(
            KeybindingManager::action_to_name(&Action::NewSession),
            "new_session"
        );
        assert_eq!(
            KeybindingManager::action_to_name(&Action::SwitchSession(3)),
            "switch_session_3"
        );
        assert_eq!(
            KeybindingManager::action_to_name(&Action::Custom("foo".to_string())),
            "custom:foo"
        );
    }

    #[test]
    fn test_action_serialization() {
        let action = Action::SwitchSession(5);
        let json = serde_json::to_string(&action).unwrap();
        let parsed: Action = serde_json::from_str(&json).unwrap();
        assert_eq!(action, parsed);
    }

    #[test]
    fn test_action_equality() {
        assert_eq!(Action::NewSession, Action::NewSession);
        assert_ne!(Action::NewSession, Action::CloseSession);
        assert_eq!(Action::SwitchSession(1), Action::SwitchSession(1));
        assert_ne!(Action::SwitchSession(1), Action::SwitchSession(2));
    }

    #[test]
    fn test_action_clone() {
        let action = Action::Custom("test".to_string());
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }

    #[test]
    fn test_keybinding_manager_default() {
        let manager = KeybindingManager::default();
        #[cfg(target_os = "macos")]
        let key = "Cmd+Shift+N";
        #[cfg(not(target_os = "macos"))]
        let key = "Ctrl+Shift+N";
        let binding = KeybindingManager::parse_binding(key).unwrap();
        assert_eq!(manager.get_action(&binding), Some(&Action::NewSession));
    }

    #[test]
    fn test_keybinding_manager_clone() {
        let manager = KeybindingManager::with_defaults();
        let cloned = manager.clone();
        #[cfg(target_os = "macos")]
        let key = "Cmd+Shift+N";
        #[cfg(not(target_os = "macos"))]
        let key = "Ctrl+Shift+N";
        let binding = KeybindingManager::parse_binding(key).unwrap();
        assert_eq!(cloned.get_action(&binding), Some(&Action::NewSession));
    }

    #[test]
    fn test_reverse_map_cleanup() {
        let mut manager = KeybindingManager::new();
        let binding = KeyBinding::with_cmd("X");

        // Add binding
        manager.set_binding(binding.clone(), Action::Quit);
        assert!(!manager.get_bindings(&Action::Quit).is_empty());

        // Replace with different action
        manager.set_binding(binding.clone(), Action::Copy);

        // Old action should have no bindings
        assert!(manager.get_bindings(&Action::Quit).is_empty());
        // New action should have the binding
        assert!(!manager.get_bindings(&Action::Copy).is_empty());
    }

    #[test]
    fn test_session_number_bindings() {
        let manager = KeybindingManager::with_defaults();
        #[cfg(target_os = "macos")]
        let m = "Cmd";
        #[cfg(not(target_os = "macos"))]
        let m = "Ctrl";

        for i in 1..=9 {
            let binding = KeybindingManager::parse_binding(&format!("{m}+{i}")).unwrap();
            assert_eq!(
                manager.get_action(&binding),
                Some(&Action::SwitchSession(i))
            );
        }
    }

    #[test]
    fn test_keybinding_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        let kb1 = KeyBinding::with_cmd("N");
        let kb2 = KeyBinding::with_cmd("N");
        set.insert(kb1);
        assert!(set.contains(&kb2));
    }

    #[test]
    fn test_action_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Action::NewSession);
        assert!(set.contains(&Action::NewSession));
    }
}
