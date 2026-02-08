//! Advanced theme configuration system for Codirigent.
//!
//! This module provides serializable theme definitions that can be loaded
//! from JSON configuration files, enabling custom themes.
//!
//! # Overview
//!
//! The theme system provides:
//! - [`Theme`] - Complete theme definition with colors, typography, and spacing
//! - [`ThemeColors`] - Color palette for all UI elements
//! - [`TerminalColors`] - ANSI 16-color palette for terminals
//! - [`ThemeTypography`] - Font settings
//! - [`ThemeSpacing`] - Layout spacing values
//!
//! # Default Themes
//!
//! Two built-in themes are provided:
//! - Dark theme (default) - Based on the spec's dark palette
//! - Light theme - High contrast light mode
//!
//! # Custom Themes
//!
//! Custom themes can be loaded from JSON files:
//!
//! ```
//! use codirigent_ui::theme_config::Theme;
//!
//! let json = r#"{"id": "custom", "name": "Custom", "is_dark": true, ...}"#;
//! // let theme = Theme::from_json(json).unwrap();
//! ```

use serde::{Deserialize, Serialize};
use serde_json;

/// Color value in hex format (e.g., "#1a1a2e").
pub type HexColor = String;

/// Complete theme definition.
///
/// A theme contains all visual settings for the Codirigent UI, including
/// colors, typography, and spacing values.
///
/// # Example
///
/// ```
/// use codirigent_ui::theme_config::Theme;
///
/// let dark = Theme::dark();
/// assert!(dark.is_dark);
/// assert_eq!(dark.id, "dark");
///
/// let light = Theme::light();
/// assert!(!light.is_dark);
/// assert_eq!(light.id, "light");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    /// Theme identifier (unique).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Whether this is a dark theme.
    pub is_dark: bool,
    /// Color palette.
    pub colors: ThemeColors,
    /// Typography settings.
    pub typography: ThemeTypography,
    /// Spacing values.
    pub spacing: ThemeSpacing,
}

/// Theme color palette.
///
/// Contains all colors used throughout the UI, organized by purpose.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeColors {
    // Background colors
    /// Primary background color (main window).
    pub background_primary: HexColor,
    /// Secondary background color (panels).
    pub background_secondary: HexColor,
    /// Tertiary background color (nested elements).
    pub background_tertiary: HexColor,

    // Foreground colors
    /// Primary text color.
    pub foreground_primary: HexColor,
    /// Secondary text color.
    pub foreground_secondary: HexColor,
    /// Muted/disabled text color.
    pub foreground_muted: HexColor,

    // Accent colors
    /// Primary accent color (buttons, links).
    pub accent_primary: HexColor,
    /// Secondary accent color (hover states).
    pub accent_secondary: HexColor,

    // Status colors
    /// Color for idle sessions.
    pub status_idle: HexColor,
    /// Color for working/active sessions.
    pub status_working: HexColor,
    /// Color for sessions waiting for input.
    pub status_waiting: HexColor,
    /// Color for completed sessions.
    pub status_done: HexColor,
    /// Color for sessions with errors.
    pub status_error: HexColor,

    // Session group colors (predefined palette)
    /// Colors for session grouping.
    pub group_colors: Vec<HexColor>,

    // Border colors
    /// Primary border color.
    pub border_primary: HexColor,
    /// Focused element border color.
    pub border_focused: HexColor,

    // Terminal colors (ANSI 16-color palette)
    /// Terminal ANSI color palette.
    pub terminal: TerminalColors,
}

/// Terminal ANSI colors.
///
/// The standard 16-color ANSI palette used for terminal rendering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalColors {
    /// Black (color 0).
    pub black: HexColor,
    /// Red (color 1).
    pub red: HexColor,
    /// Green (color 2).
    pub green: HexColor,
    /// Yellow (color 3).
    pub yellow: HexColor,
    /// Blue (color 4).
    pub blue: HexColor,
    /// Magenta (color 5).
    pub magenta: HexColor,
    /// Cyan (color 6).
    pub cyan: HexColor,
    /// White (color 7).
    pub white: HexColor,
    /// Bright black (color 8).
    pub bright_black: HexColor,
    /// Bright red (color 9).
    pub bright_red: HexColor,
    /// Bright green (color 10).
    pub bright_green: HexColor,
    /// Bright yellow (color 11).
    pub bright_yellow: HexColor,
    /// Bright blue (color 12).
    pub bright_blue: HexColor,
    /// Bright magenta (color 13).
    pub bright_magenta: HexColor,
    /// Bright cyan (color 14).
    pub bright_cyan: HexColor,
    /// Bright white (color 15).
    pub bright_white: HexColor,
}

/// Typography settings.
///
/// Font family and size settings for UI and terminal rendering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeTypography {
    /// Main UI font family.
    pub ui_font_family: String,
    /// Terminal font family.
    pub terminal_font_family: String,
    /// Base font size in pixels.
    pub base_font_size: f32,
    /// Terminal font size in pixels.
    pub terminal_font_size: f32,
    /// Line height multiplier.
    pub line_height: f32,
}

/// Spacing values.
///
/// Standard spacing values used throughout the UI for consistent layout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeSpacing {
    /// Extra small spacing (2px).
    pub xs: f32,
    /// Small spacing (4px).
    pub sm: f32,
    /// Medium spacing (8px).
    pub md: f32,
    /// Large spacing (16px).
    pub lg: f32,
    /// Extra large spacing (24px).
    pub xl: f32,
    /// Grid gap between sessions.
    pub grid_gap: f32,
    /// Border radius.
    pub border_radius: f32,
}

impl Theme {
    /// Create the default dark theme.
    ///
    /// Uses colors from the Codirigent spec with a dark background.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::theme_config::Theme;
    ///
    /// let theme = Theme::dark();
    /// assert!(theme.is_dark);
    /// assert_eq!(theme.colors.background_primary, "#1a1a2e");
    /// ```
    pub fn dark() -> Self {
        Self {
            id: "dark".to_string(),
            name: "Dark".to_string(),
            is_dark: true,
            colors: ThemeColors {
                background_primary: "#1a1a2e".to_string(),
                background_secondary: "#16213e".to_string(),
                background_tertiary: "#0f3460".to_string(),
                foreground_primary: "#eaeaea".to_string(),
                foreground_secondary: "#b8b8b8".to_string(),
                foreground_muted: "#6b6b6b".to_string(),
                accent_primary: "#e94560".to_string(),
                accent_secondary: "#0f3460".to_string(),
                status_idle: "#6b6b6b".to_string(),
                status_working: "#f39c12".to_string(),
                status_waiting: "#e74c3c".to_string(),
                status_done: "#27ae60".to_string(),
                status_error: "#e74c3c".to_string(),
                group_colors: vec![
                    "#27ae60".to_string(), // green
                    "#3498db".to_string(), // blue
                    "#f39c12".to_string(), // yellow
                    "#9b59b6".to_string(), // purple
                    "#e74c3c".to_string(), // red
                    "#1abc9c".to_string(), // teal
                ],
                border_primary: "#2a2a4a".to_string(),
                border_focused: "#e94560".to_string(),
                terminal: TerminalColors::default_dark(),
            },
            typography: ThemeTypography {
                ui_font_family: "Inter".to_string(),
                terminal_font_family: "JetBrains Mono".to_string(),
                base_font_size: 14.0,
                terminal_font_size: 14.0,
                line_height: 1.5,
            },
            spacing: ThemeSpacing {
                xs: 2.0,
                sm: 4.0,
                md: 8.0,
                lg: 16.0,
                xl: 24.0,
                grid_gap: 4.0,
                border_radius: 4.0,
            },
        }
    }

    /// Create the default light theme.
    ///
    /// High contrast light mode with dark text on light backgrounds.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::theme_config::Theme;
    ///
    /// let theme = Theme::light();
    /// assert!(!theme.is_dark);
    /// assert_eq!(theme.colors.background_primary, "#ffffff");
    /// ```
    pub fn light() -> Self {
        Self {
            id: "light".to_string(),
            name: "Light".to_string(),
            is_dark: false,
            colors: ThemeColors {
                background_primary: "#ffffff".to_string(),
                background_secondary: "#f5f5f5".to_string(),
                background_tertiary: "#e0e0e0".to_string(),
                foreground_primary: "#1a1a1a".to_string(),
                foreground_secondary: "#4a4a4a".to_string(),
                foreground_muted: "#9a9a9a".to_string(),
                accent_primary: "#0066cc".to_string(),
                accent_secondary: "#e6f0ff".to_string(),
                status_idle: "#9a9a9a".to_string(),
                status_working: "#f39c12".to_string(),
                status_waiting: "#e74c3c".to_string(),
                status_done: "#27ae60".to_string(),
                status_error: "#e74c3c".to_string(),
                group_colors: vec![
                    "#27ae60".to_string(),
                    "#3498db".to_string(),
                    "#f39c12".to_string(),
                    "#9b59b6".to_string(),
                    "#e74c3c".to_string(),
                    "#1abc9c".to_string(),
                ],
                border_primary: "#d0d0d0".to_string(),
                border_focused: "#0066cc".to_string(),
                terminal: TerminalColors::default_light(),
            },
            typography: ThemeTypography {
                ui_font_family: "Inter".to_string(),
                terminal_font_family: "JetBrains Mono".to_string(),
                base_font_size: 14.0,
                terminal_font_size: 14.0,
                line_height: 1.5,
            },
            spacing: ThemeSpacing {
                xs: 2.0,
                sm: 4.0,
                md: 8.0,
                lg: 16.0,
                xl: 24.0,
                grid_gap: 4.0,
                border_radius: 4.0,
            },
        }
    }

    /// Load a custom theme from JSON.
    ///
    /// # Arguments
    ///
    /// * `json` - JSON string containing theme definition
    ///
    /// # Returns
    ///
    /// The parsed theme or a serde_json error.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::theme_config::Theme;
    ///
    /// let dark = Theme::dark();
    /// let json = serde_json::to_string_pretty(&dark).unwrap();
    /// let loaded = Theme::from_json(&json).unwrap();
    /// assert_eq!(loaded.id, "dark");
    /// ```
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize theme to JSON.
    ///
    /// # Returns
    ///
    /// Pretty-printed JSON string or serialization error.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_ui::theme_config::Theme;
    ///
    /// let theme = Theme::dark();
    /// let json = theme.to_json().unwrap();
    /// assert!(json.contains("\"id\": \"dark\""));
    /// ```
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl TerminalColors {
    /// Create the default dark terminal colors.
    pub fn default_dark() -> Self {
        Self {
            black: "#000000".to_string(),
            red: "#e74c3c".to_string(),
            green: "#27ae60".to_string(),
            yellow: "#f39c12".to_string(),
            blue: "#3498db".to_string(),
            magenta: "#9b59b6".to_string(),
            cyan: "#1abc9c".to_string(),
            white: "#ecf0f1".to_string(),
            bright_black: "#7f8c8d".to_string(),
            bright_red: "#ff6b6b".to_string(),
            bright_green: "#2ecc71".to_string(),
            bright_yellow: "#f1c40f".to_string(),
            bright_blue: "#5dade2".to_string(),
            bright_magenta: "#bb8fce".to_string(),
            bright_cyan: "#48c9b0".to_string(),
            bright_white: "#ffffff".to_string(),
        }
    }

    /// Create the default light terminal colors.
    pub fn default_light() -> Self {
        Self {
            black: "#2c3e50".to_string(),
            red: "#c0392b".to_string(),
            green: "#27ae60".to_string(),
            yellow: "#f39c12".to_string(),
            blue: "#2980b9".to_string(),
            magenta: "#8e44ad".to_string(),
            cyan: "#16a085".to_string(),
            white: "#bdc3c7".to_string(),
            bright_black: "#7f8c8d".to_string(),
            bright_red: "#e74c3c".to_string(),
            bright_green: "#2ecc71".to_string(),
            bright_yellow: "#f1c40f".to_string(),
            bright_blue: "#3498db".to_string(),
            bright_magenta: "#9b59b6".to_string(),
            bright_cyan: "#1abc9c".to_string(),
            bright_white: "#ecf0f1".to_string(),
        }
    }

    /// Get color by ANSI index (0-15).
    ///
    /// # Arguments
    ///
    /// * `index` - ANSI color index (0-15)
    ///
    /// # Returns
    ///
    /// The hex color string, or None if index is out of range.
    pub fn get(&self, index: u8) -> Option<&str> {
        match index {
            0 => Some(&self.black),
            1 => Some(&self.red),
            2 => Some(&self.green),
            3 => Some(&self.yellow),
            4 => Some(&self.blue),
            5 => Some(&self.magenta),
            6 => Some(&self.cyan),
            7 => Some(&self.white),
            8 => Some(&self.bright_black),
            9 => Some(&self.bright_red),
            10 => Some(&self.bright_green),
            11 => Some(&self.bright_yellow),
            12 => Some(&self.bright_blue),
            13 => Some(&self.bright_magenta),
            14 => Some(&self.bright_cyan),
            15 => Some(&self.bright_white),
            _ => None,
        }
    }
}

impl Default for TerminalColors {
    fn default() -> Self {
        Self::default_dark()
    }
}

impl Default for ThemeTypography {
    fn default() -> Self {
        Self {
            ui_font_family: "Inter".to_string(),
            terminal_font_family: "JetBrains Mono".to_string(),
            base_font_size: 14.0,
            terminal_font_size: 14.0,
            line_height: 1.5,
        }
    }
}

impl Default for ThemeSpacing {
    fn default() -> Self {
        Self {
            xs: 2.0,
            sm: 4.0,
            md: 8.0,
            lg: 16.0,
            xl: 24.0,
            grid_gap: 4.0,
            border_radius: 4.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme() {
        let theme = Theme::dark();
        assert!(theme.is_dark);
        assert_eq!(theme.id, "dark");
        assert_eq!(theme.name, "Dark");
    }

    #[test]
    fn test_light_theme() {
        let theme = Theme::light();
        assert!(!theme.is_dark);
        assert_eq!(theme.id, "light");
        assert_eq!(theme.name, "Light");
    }

    #[test]
    fn test_theme_default() {
        let theme = Theme::default();
        assert!(theme.is_dark);
        assert_eq!(theme.id, "dark");
    }

    #[test]
    fn test_theme_serialization() {
        let theme = Theme::dark();
        let json = serde_json::to_string_pretty(&theme).unwrap();
        let loaded = Theme::from_json(&json).unwrap();
        assert_eq!(loaded.id, theme.id);
        assert_eq!(loaded.is_dark, theme.is_dark);
        assert_eq!(
            loaded.colors.background_primary,
            theme.colors.background_primary
        );
    }

    #[test]
    fn test_theme_to_json() {
        let theme = Theme::dark();
        let json = theme.to_json().unwrap();
        assert!(json.contains("\"id\": \"dark\""));
        assert!(json.contains("\"is_dark\": true"));
    }

    #[test]
    fn test_theme_colors_dark() {
        let theme = Theme::dark();
        assert_eq!(theme.colors.background_primary, "#1a1a2e");
        assert_eq!(theme.colors.foreground_primary, "#eaeaea");
        assert_eq!(theme.colors.accent_primary, "#e94560");
    }

    #[test]
    fn test_theme_colors_light() {
        let theme = Theme::light();
        assert_eq!(theme.colors.background_primary, "#ffffff");
        assert_eq!(theme.colors.foreground_primary, "#1a1a1a");
        assert_eq!(theme.colors.accent_primary, "#0066cc");
    }

    #[test]
    fn test_status_colors() {
        let theme = Theme::dark();
        assert_eq!(theme.colors.status_idle, "#6b6b6b");
        assert_eq!(theme.colors.status_working, "#f39c12");
        assert_eq!(theme.colors.status_waiting, "#e74c3c");
        assert_eq!(theme.colors.status_done, "#27ae60");
        assert_eq!(theme.colors.status_error, "#e74c3c");
    }

    #[test]
    fn test_group_colors() {
        let theme = Theme::dark();
        assert_eq!(theme.colors.group_colors.len(), 6);
        assert_eq!(theme.colors.group_colors[0], "#27ae60");
    }

    #[test]
    fn test_terminal_colors_dark() {
        let colors = TerminalColors::default_dark();
        assert_eq!(colors.black, "#000000");
        assert_eq!(colors.red, "#e74c3c");
        assert_eq!(colors.bright_white, "#ffffff");
    }

    #[test]
    fn test_terminal_colors_light() {
        let colors = TerminalColors::default_light();
        assert_eq!(colors.black, "#2c3e50");
        assert_eq!(colors.red, "#c0392b");
    }

    #[test]
    fn test_terminal_colors_default() {
        let colors = TerminalColors::default();
        assert_eq!(colors.black, "#000000"); // Same as dark
    }

    #[test]
    fn test_terminal_colors_get() {
        let colors = TerminalColors::default_dark();
        assert_eq!(colors.get(0), Some("#000000"));
        assert_eq!(colors.get(1), Some("#e74c3c"));
        assert_eq!(colors.get(15), Some("#ffffff"));
        assert_eq!(colors.get(16), None);
    }

    #[test]
    fn test_terminal_colors_get_all_indices() {
        let colors = TerminalColors::default_dark();
        for i in 0..16 {
            assert!(colors.get(i).is_some(), "Color {} should exist", i);
        }
    }

    #[test]
    fn test_typography_default() {
        let typo = ThemeTypography::default();
        assert_eq!(typo.ui_font_family, "Inter");
        assert_eq!(typo.terminal_font_family, "JetBrains Mono");
        assert_eq!(typo.base_font_size, 14.0);
        assert_eq!(typo.terminal_font_size, 14.0);
        assert_eq!(typo.line_height, 1.5);
    }

    #[test]
    fn test_typography_serialization() {
        let typo = ThemeTypography::default();
        let json = serde_json::to_string(&typo).unwrap();
        let parsed: ThemeTypography = serde_json::from_str(&json).unwrap();
        assert_eq!(typo.ui_font_family, parsed.ui_font_family);
        assert_eq!(typo.base_font_size, parsed.base_font_size);
    }

    #[test]
    fn test_spacing_default() {
        let spacing = ThemeSpacing::default();
        assert_eq!(spacing.xs, 2.0);
        assert_eq!(spacing.sm, 4.0);
        assert_eq!(spacing.md, 8.0);
        assert_eq!(spacing.lg, 16.0);
        assert_eq!(spacing.xl, 24.0);
        assert_eq!(spacing.grid_gap, 4.0);
        assert_eq!(spacing.border_radius, 4.0);
    }

    #[test]
    fn test_spacing_serialization() {
        let spacing = ThemeSpacing::default();
        let json = serde_json::to_string(&spacing).unwrap();
        let parsed: ThemeSpacing = serde_json::from_str(&json).unwrap();
        assert_eq!(spacing.xs, parsed.xs);
        assert_eq!(spacing.grid_gap, parsed.grid_gap);
    }

    #[test]
    fn test_theme_colors_equality() {
        let theme1 = Theme::dark();
        let theme2 = Theme::dark();
        assert_eq!(theme1.colors, theme2.colors);
    }

    #[test]
    fn test_theme_clone() {
        let theme = Theme::dark();
        let cloned = theme.clone();
        assert_eq!(theme, cloned);
    }

    #[test]
    fn test_theme_debug() {
        let theme = Theme::dark();
        let debug_str = format!("{:?}", theme);
        assert!(debug_str.contains("Theme"));
        assert!(debug_str.contains("dark"));
    }

    #[test]
    fn test_invalid_json() {
        let result = Theme::from_json("invalid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_partial_json() {
        // Missing required fields should fail
        let result = Theme::from_json(r#"{"id": "test"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_theme_roundtrip() {
        let original = Theme::light();
        let json = original.to_json().unwrap();
        let loaded = Theme::from_json(&json).unwrap();
        assert_eq!(original, loaded);
    }

    #[test]
    fn test_terminal_colors_clone() {
        let colors = TerminalColors::default_dark();
        let cloned = colors.clone();
        assert_eq!(colors, cloned);
    }

    #[test]
    fn test_theme_colors_clone() {
        let theme = Theme::dark();
        let cloned = theme.colors.clone();
        assert_eq!(theme.colors.background_primary, cloned.background_primary);
    }

    #[test]
    fn test_typography_clone() {
        let typo = ThemeTypography::default();
        let cloned = typo.clone();
        assert_eq!(typo, cloned);
    }

    #[test]
    fn test_spacing_clone() {
        let spacing = ThemeSpacing::default();
        let cloned = spacing.clone();
        assert_eq!(spacing, cloned);
    }
}
