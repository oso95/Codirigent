//! Theme system for Codirigent.
//!
//! Provides color themes for the Codirigent UI, including dark and light modes.
//! The dark theme uses a custom color palette designed for the Dirigent dashboard.
//!
//! # Color Types
//!
//! This module provides two color representations:
//! - [`Hsla`] - Hue-Saturation-Lightness-Alpha for UI elements (GPUI compatible)
//! - [`Rgba`] - Red-Green-Blue-Alpha for terminal colors (alacritty_terminal compatible)

use codirigent_core::SessionStatus;

#[cfg(feature = "gpui-full")]
use gpui::Hsla as GpuiHsla;

/// RGBA color representation.
///
/// Components are stored as u8 values (0-255). Used for terminal cell colors
/// which need to interface with alacritty_terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    /// Red component (0-255).
    pub r: u8,
    /// Green component (0-255).
    pub g: u8,
    /// Blue component (0-255).
    pub b: u8,
    /// Alpha component (0-255, 255 = fully opaque).
    pub a: u8,
}

impl Rgba {
    /// Create a new RGBA color.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create an opaque RGB color (alpha = 255).
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// Convert to HSLA.
    pub fn to_hsla(&self) -> Hsla {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;
        let a = self.a as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        if (max - min).abs() < f32::EPSILON {
            return Hsla::new(0.0, 0.0, l, a);
        }

        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };

        let h = if (max - r).abs() < f32::EPSILON {
            ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
        } else if (max - g).abs() < f32::EPSILON {
            ((b - r) / d + 2.0) / 6.0
        } else {
            ((r - g) / d + 4.0) / 6.0
        };

        Hsla::new(h, s, l, a)
    }
}

impl Default for Rgba {
    fn default() -> Self {
        Self::rgb(0, 0, 0)
    }
}

/// Standard ANSI terminal colors (16 colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnsiColors {
    /// Colors 0-15 in order.
    pub colors: [Rgba; 16],
}

impl AnsiColors {
    /// Get color by index (0-15).
    pub fn get(&self, index: u8) -> Option<Rgba> {
        if index < 16 {
            Some(self.colors[index as usize])
        } else {
            None
        }
    }
}

impl Default for AnsiColors {
    fn default() -> Self {
        Self {
            colors: [
                Rgba::rgb(0, 0, 0),       // Black
                Rgba::rgb(204, 0, 0),     // Red
                Rgba::rgb(0, 204, 0),     // Green
                Rgba::rgb(204, 204, 0),   // Yellow
                Rgba::rgb(0, 0, 204),     // Blue
                Rgba::rgb(204, 0, 204),   // Magenta
                Rgba::rgb(0, 204, 204),   // Cyan
                Rgba::rgb(204, 204, 204), // White
                Rgba::rgb(128, 128, 128), // Bright Black
                Rgba::rgb(255, 0, 0),     // Bright Red
                Rgba::rgb(0, 255, 0),     // Bright Green
                Rgba::rgb(255, 255, 0),   // Bright Yellow
                Rgba::rgb(0, 0, 255),     // Bright Blue
                Rgba::rgb(255, 0, 255),   // Bright Magenta
                Rgba::rgb(0, 255, 255),   // Bright Cyan
                Rgba::rgb(255, 255, 255), // Bright White
            ],
        }
    }
}

/// HSLA color representation.
///
/// Hue-Saturation-Lightness-Alpha color model, compatible with GPUI's
/// color system when the `gpui` feature is enabled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsla {
    /// Hue (0.0 to 1.0).
    pub h: f32,
    /// Saturation (0.0 to 1.0).
    pub s: f32,
    /// Lightness (0.0 to 1.0).
    pub l: f32,
    /// Alpha (0.0 to 1.0).
    pub a: f32,
}

impl Hsla {
    /// Create a new HSLA color.
    pub const fn new(h: f32, s: f32, l: f32, a: f32) -> Self {
        Self { h, s, l, a }
    }
}

/// Convert theme Hsla to GPUI Hsla.
#[cfg(feature = "gpui-full")]
impl From<Hsla> for GpuiHsla {
    fn from(color: Hsla) -> Self {
        GpuiHsla {
            h: color.h,
            s: color.s,
            l: color.l,
            a: color.a,
        }
    }
}

/// Convert GPUI Hsla to theme Hsla.
#[cfg(feature = "gpui-full")]
impl From<GpuiHsla> for Hsla {
    fn from(color: GpuiHsla) -> Self {
        Hsla {
            h: color.h,
            s: color.s,
            l: color.l,
            a: color.a,
        }
    }
}

/// Convert theme Rgba to GPUI Hsla.
#[cfg(feature = "gpui-full")]
impl From<Rgba> for GpuiHsla {
    fn from(color: Rgba) -> Self {
        let hsla = color.to_hsla();
        GpuiHsla {
            h: hsla.h,
            s: hsla.s,
            l: hsla.l,
            a: hsla.a,
        }
    }
}

/// Helper function to create an HSLA color.
///
/// # Arguments
///
/// * `h` - Hue (0.0 to 1.0)
/// * `s` - Saturation (0.0 to 1.0)
/// * `l` - Lightness (0.0 to 1.0)
/// * `a` - Alpha (0.0 to 1.0)
pub const fn hsla(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla::new(h, s, l, a)
}

/// Convert a hex color string to HSLA.
///
/// Accepts formats: "#RGB", "#RRGGBB", "RGB", "RRGGBB"
///
/// # Arguments
///
/// * `hex` - Hex color string
///
/// # Returns
///
/// HSLA color or None if parsing fails
pub fn hex_to_hsla(hex: &str) -> Option<Hsla> {
    let hex = hex.trim_start_matches('#');
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some(Rgba::rgb(r, g, b).to_hsla())
}

/// Convert a hex color string to HSLA, panicking if invalid.
///
/// Use only with known-valid hex strings (e.g., compile-time constants).
fn hex(hex: &str) -> Hsla {
    hex_to_hsla(hex).expect("Invalid hex color")
}

/// Codirigent color theme.
///
/// Contains all colors used throughout the Codirigent UI. Each color is defined
/// as an HSLA value for flexibility in rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct CodirigentTheme {
    // === Background Colors ===
    /// Main/deepest background color.
    pub background: Hsla,
    /// Panel background color (slightly lighter than background).
    pub panel_background: Hsla,
    /// Header/toolbar background color.
    pub header_background: Hsla,
    /// Sidebar background color.
    pub sidebar_background: Hsla,

    // === Border & Interaction Colors ===
    /// Border color for panels and dividers.
    pub border: Hsla,
    /// Hover state background color.
    pub hover: Hsla,
    /// Active/focused element background.
    pub active: Hsla,
    /// Selection highlight color.
    pub selection: Hsla,

    // === Text Colors ===
    /// Primary text color.
    pub foreground: Hsla,
    /// Secondary text color.
    pub text_secondary: Hsla,
    /// Muted/tertiary text color.
    pub muted: Hsla,

    // === Accent Colors ===
    /// Primary accent color (indigo).
    pub primary: Hsla,
    /// Secondary accent color (indigo-light).
    pub secondary: Hsla,
    /// Purple accent color.
    pub purple: Hsla,
    /// Orange accent color.
    pub orange: Hsla,

    // === Mockup-Specific Colors ===
    /// Icon rail background (narrow left bar).
    pub icon_rail_background: Hsla,
    /// Drawer panel background (expandable left panel).
    pub drawer_background: Hsla,
    /// Selected/focused session ring color.
    pub selected_ring: Hsla,
    /// Broadcast mode accent color (rose).
    pub broadcast_accent: Hsla,
    /// AI summary pill background (indigo tint).
    pub ai_summary_background: Hsla,
    /// AI summary pill text color.
    pub ai_summary_text: Hsla,
    /// Input required overlay background.
    pub input_required_background: Hsla,
    /// Input required accent color.
    pub input_required_accent: Hsla,

    // === Session Status Colors ===
    /// Color for idle sessions.
    pub session_idle: Hsla,
    /// Color for working/active sessions.
    pub session_working: Hsla,
    /// Color for sessions needing attention (input or permission).
    pub session_needs_attention: Hsla,
    /// Color for sessions with errors.
    pub session_error: Hsla,

    // === Priority Colors ===
    /// High priority color.
    pub priority_high: Hsla,
    /// Medium priority color.
    pub priority_medium: Hsla,
    /// Low priority color.
    pub priority_low: Hsla,

    // === Session Group Colors ===
    /// Session group colors (for visual grouping).
    pub session_colors: [Hsla; 6],

    // === Terminal Colors ===
    /// Cursor color in terminals.
    pub cursor: Hsla,
    /// Terminal ANSI colors.
    pub ansi: AnsiColors,
    /// Terminal background as RGBA.
    pub terminal_background: Rgba,
    /// Terminal foreground as RGBA.
    pub terminal_foreground: Rgba,
    /// Terminal cursor as RGBA.
    pub terminal_cursor: Rgba,
    /// Terminal selection background as RGBA.
    pub terminal_selection_bg: Rgba,
    /// Terminal selection foreground as RGBA.
    pub terminal_selection_fg: Rgba,

    // === Layout ===
    /// Gap between grid cells in pixels.
    pub grid_gap: f32,

    // === Typography ===
    /// Base font size for UI text.
    pub font_size_base: f32,
    /// Small font size.
    pub font_size_small: f32,
    /// Large font size.
    pub font_size_large: f32,
    /// Terminal font size (separate from UI font size).
    pub terminal_font_size: f32,

    // === Spacing ===
    /// Base spacing unit in pixels.
    pub spacing_base: f32,
    /// Small spacing.
    pub spacing_small: f32,
    /// Large spacing.
    pub spacing_large: f32,
}

impl CodirigentTheme {
    /// Create the dark theme.
    ///
    /// Uses the Dirigent dashboard color palette.
    pub fn dark() -> Self {
        Self {
            // === Background Colors (from mockup) ===
            background: hex("#050505"),         // Darkest background
            panel_background: hex("#0c0c0e"),   // Panel background
            header_background: hex("#09090b"),  // Header/toolbar background
            sidebar_background: hex("#0c0c0e"), // Same as panel

            // === Border & Interaction Colors ===
            border: hex("#1a1a1f"), // Border color
            hover: hex("#151518"),  // Hover state
            active: hex("#1a1a22"), // Active/focused state
            selection: Hsla {
                a: 0.3,
                ..hex("#6366f1")
            }, // Primary @ 30%

            // === Text Colors ===
            foreground: hex("#e0e0e0"),     // Primary text
            text_secondary: hex("#888888"), // Secondary text
            muted: hex("#555555"),          // Muted text

            // === Accent Colors ===
            primary: hex("#6366f1"),   // Indigo-500 (main accent)
            secondary: hex("#818cf8"), // Indigo-400
            purple: hex("#A78BFA"),    // Purple
            orange: hex("#F59E0B"),    // Orange

            // === Mockup-Specific Colors ===
            icon_rail_background: hex("#0c0c0e"),
            drawer_background: hex("#121214"),
            selected_ring: hex("#6366f1"),
            broadcast_accent: hex("#f43f5e"),
            ai_summary_background: Hsla {
                a: 0.05,
                ..hex("#6366f1")
            },
            ai_summary_text: Hsla {
                a: 0.8,
                ..hex("#c7d2fe")
            },
            input_required_background: Hsla {
                a: 0.2,
                ..hex("#4c0519")
            },
            input_required_accent: hex("#f43f5e"),

            // === Session Status Colors ===
            session_idle: hex("#52525b"),            // Zinc-600 for idle
            session_working: hex("#f59e0b"),         // Amber-500 for working
            session_needs_attention: hex("#f43f5e"), // Rose-500 for needs attention
            session_error: hex("#ef4444"),           // Red-500 for error

            // === Priority Colors ===
            priority_high: hex("#FF6B6B"),   // Red
            priority_medium: hex("#F59E0B"), // Orange
            priority_low: hex("#5B8DEF"),    // Blue

            // === Session Group Colors ===
            session_colors: [
                hex("#6366f1"), // Indigo
                hex("#818cf8"), // Indigo-400
                hex("#A78BFA"), // Purple
                hex("#F59E0B"), // Orange
                hex("#f43f5e"), // Rose
                hex("#10B981"), // Green
            ],

            // === Terminal Colors ===
            cursor: hex("#6366f1"), // Indigo cursor
            ansi: AnsiColors::default(),
            terminal_background: Rgba::rgb(5, 5, 5), // #050505
            terminal_foreground: Rgba::rgb(224, 224, 224), // #e0e0e0
            terminal_cursor: Rgba::rgb(99, 102, 241), // #6366f1
            terminal_selection_bg: Rgba::new(99, 102, 241, 77), // #6366f1 @ 30%
            terminal_selection_fg: Rgba::rgb(224, 224, 224), // #e0e0e0

            // === Layout ===
            grid_gap: 4.0,

            // === Typography ===
            font_size_base: 13.0,
            font_size_small: 11.0,
            font_size_large: 15.0,
            terminal_font_size: 13.0,

            // === Spacing ===
            spacing_base: 8.0,
            spacing_small: 4.0,
            spacing_large: 16.0,
        }
    }

    /// Create the light theme.
    ///
    /// Light theme variant with inverted colors for better readability
    /// in bright environments.
    pub fn light() -> Self {
        Self {
            // === Background Colors ===
            background: hex("#f5f5f7"),
            panel_background: hex("#ffffff"),
            header_background: hex("#e8e8ec"),
            sidebar_background: hex("#f0f0f4"),

            // === Border & Interaction Colors ===
            border: hex("#d0d0d8"),
            hover: hex("#e8e8ec"),
            active: hex("#d8d8e0"),
            selection: Hsla {
                a: 0.2,
                ..hex("#4f46e5")
            }, // Indigo-600 @ 20%

            // === Text Colors ===
            foreground: hex("#1a1a1c"),
            text_secondary: hex("#666666"),
            muted: hex("#999999"),

            // === Accent Colors (slightly darker for light bg) ===
            primary: hex("#4f46e5"),   // Indigo-600
            secondary: hex("#6366f1"), // Indigo-500
            purple: hex("#8B6FD9"),    // Darker purple
            orange: hex("#D98A0B"),    // Darker orange

            // === Mockup-Specific Colors ===
            icon_rail_background: hex("#f0f0f4"),
            drawer_background: hex("#ffffff"),
            selected_ring: hex("#4f46e5"),
            broadcast_accent: hex("#e11d48"),
            ai_summary_background: Hsla {
                a: 0.08,
                ..hex("#4f46e5")
            },
            ai_summary_text: hex("#3730a3"),
            input_required_background: Hsla {
                a: 0.1,
                ..hex("#e11d48")
            },
            input_required_accent: hex("#e11d48"),

            // === Session Status Colors ===
            session_idle: hex("#71717a"),            // Zinc-500
            session_working: hex("#d97706"),         // Amber-600
            session_needs_attention: hex("#e11d48"), // Rose-600
            session_error: hex("#dc2626"),           // Red-600

            // === Priority Colors ===
            priority_high: hex("#dc2626"),
            priority_medium: hex("#D98A0B"),
            priority_low: hex("#6366f1"),

            // === Session Group Colors ===
            session_colors: [
                hex("#4f46e5"), // Indigo-600
                hex("#6366f1"), // Indigo-500
                hex("#8B6FD9"), // Purple
                hex("#D98A0B"), // Orange
                hex("#e11d48"), // Rose
                hex("#059669"), // Emerald
            ],

            // === Terminal Colors ===
            cursor: hex("#4f46e5"),
            ansi: AnsiColors::default(),
            terminal_background: Rgba::rgb(245, 245, 247),
            terminal_foreground: Rgba::rgb(26, 26, 28),
            terminal_cursor: Rgba::rgb(79, 70, 229), // #4f46e5
            terminal_selection_bg: Rgba::new(79, 70, 229, 51), // #4f46e5 @ 20%
            terminal_selection_fg: Rgba::rgb(26, 26, 28),

            // === Layout ===
            grid_gap: 4.0,

            // === Typography ===
            font_size_base: 13.0,
            font_size_small: 11.0,
            font_size_large: 15.0,
            terminal_font_size: 13.0,

            // === Spacing ===
            spacing_base: 8.0,
            spacing_small: 4.0,
            spacing_large: 16.0,
        }
    }

    /// Get the color for a given session status.
    ///
    /// Returns the appropriate status indicator color based on the session's
    /// current state.
    ///
    /// # Arguments
    ///
    /// * `status` - The session status to get the color for
    ///
    /// # Returns
    ///
    /// The HSLA color corresponding to the status
    pub fn status_color(&self, status: SessionStatus) -> Hsla {
        match status {
            SessionStatus::Idle => self.session_idle,
            SessionStatus::Working => self.session_working,
            SessionStatus::NeedsAttention => self.session_needs_attention,
            SessionStatus::Error => self.session_error,
        }
    }

    /// Get the display name for a session status.
    ///
    /// Returns a human-readable string for the status.
    ///
    /// # Arguments
    ///
    /// * `status` - The session status
    ///
    /// # Returns
    ///
    /// A string slice with the status name
    pub fn status_name(status: SessionStatus) -> &'static str {
        match status {
            SessionStatus::Idle => "Idle",
            SessionStatus::Working => "Working",
            SessionStatus::NeedsAttention => "Attention",
            SessionStatus::Error => "Error",
        }
    }

    /// Get a color from the 256-color indexed palette.
    ///
    /// The 256-color palette is organized as:
    /// - 0-15: Standard ANSI colors
    /// - 16-231: 6x6x6 color cube
    /// - 232-255: Grayscale ramp
    ///
    /// # Arguments
    ///
    /// * `index` - Color index (0-255)
    ///
    /// # Returns
    ///
    /// The RGBA color for the given index
    pub fn get_indexed_color(&self, index: u8) -> Rgba {
        match index {
            // Standard ANSI colors (0-15)
            0..=15 => self.ansi.colors[index as usize],
            // 6x6x6 color cube (16-231)
            16..=231 => {
                let idx = index - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                // Convert 0-5 to 0-255 (0, 95, 135, 175, 215, 255)
                let to_component = |v: u8| -> u8 {
                    if v == 0 {
                        0
                    } else {
                        55 + v * 40
                    }
                };
                Rgba::rgb(to_component(r), to_component(g), to_component(b))
            }
            // Grayscale ramp (232-255)
            232..=255 => {
                let gray = 8 + (index - 232) * 10;
                Rgba::rgb(gray, gray, gray)
            }
        }
    }
}

impl Default for CodirigentTheme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme_creation() {
        let theme = CodirigentTheme::dark();
        // Dark theme should have low lightness background
        assert!(
            theme.background.l < 0.1,
            "Dark bg lightness: {}",
            theme.background.l
        );
        // Dark theme should have high lightness foreground
        assert!(
            theme.foreground.l > 0.5,
            "Dark fg lightness: {}",
            theme.foreground.l
        );
    }

    #[test]
    fn test_light_theme_creation() {
        let theme = CodirigentTheme::light();
        // Light theme should have high lightness background
        assert!(
            theme.background.l > 0.9,
            "Light bg lightness: {}",
            theme.background.l
        );
        // Light theme should have low lightness foreground
        assert!(
            theme.foreground.l < 0.2,
            "Light fg lightness: {}",
            theme.foreground.l
        );
    }

    #[test]
    fn test_default_is_dark() {
        let default = CodirigentTheme::default();
        let dark = CodirigentTheme::dark();
        assert_eq!(default.background, dark.background);
    }

    #[test]
    fn test_status_colors_all_variants() {
        let theme = CodirigentTheme::dark();

        // Test all status variants return valid colors
        let statuses = [
            SessionStatus::Idle,
            SessionStatus::Working,
            SessionStatus::NeedsAttention,
            SessionStatus::Error,
        ];

        for status in statuses {
            let color = theme.status_color(status);
            // All colors should be fully opaque
            assert!(
                color.a >= 0.9,
                "Status {:?} should have alpha >= 0.9",
                status
            );
        }
    }

    #[test]
    fn test_status_colors_distinct() {
        let theme = CodirigentTheme::dark();

        // Status colors should be distinct from each other
        let working = theme.status_color(SessionStatus::Working);
        let attention = theme.status_color(SessionStatus::NeedsAttention);
        let idle = theme.status_color(SessionStatus::Idle);

        assert_ne!(
            working, attention,
            "Working and NeedsAttention should be different"
        );
        assert_ne!(idle, working, "Idle and Working should be different");
    }

    #[test]
    fn test_status_names() {
        assert_eq!(CodirigentTheme::status_name(SessionStatus::Idle), "Idle");
        assert_eq!(
            CodirigentTheme::status_name(SessionStatus::Working),
            "Working"
        );
        assert_eq!(
            CodirigentTheme::status_name(SessionStatus::NeedsAttention),
            "Attention"
        );
        assert_eq!(CodirigentTheme::status_name(SessionStatus::Error), "Error");
    }

    #[test]
    fn test_theme_clone() {
        let theme = CodirigentTheme::dark();
        let cloned = theme.clone();
        assert_eq!(theme, cloned);
    }

    #[test]
    fn test_theme_debug() {
        let theme = CodirigentTheme::dark();
        let debug_str = format!("{:?}", theme);
        assert!(debug_str.contains("CodirigentTheme"));
    }

    #[test]
    fn test_hsla_new() {
        let color = Hsla::new(0.5, 0.5, 0.5, 1.0);
        assert_eq!(color.h, 0.5);
        assert_eq!(color.s, 0.5);
        assert_eq!(color.l, 0.5);
        assert_eq!(color.a, 1.0);
    }

    #[test]
    fn test_hsla_helper() {
        let color = hsla(0.25, 0.75, 0.5, 0.8);
        assert_eq!(color.h, 0.25);
        assert_eq!(color.s, 0.75);
        assert_eq!(color.l, 0.5);
        assert_eq!(color.a, 0.8);
    }

    #[test]
    fn test_grid_gap() {
        let theme = CodirigentTheme::dark();
        assert_eq!(theme.grid_gap, 4.0);
    }

    #[test]
    fn test_rgba_new() {
        let color = Rgba::new(255, 128, 64, 255);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 64);
        assert_eq!(color.a, 255);
    }

    #[test]
    fn test_rgba_rgb() {
        let color = Rgba::rgb(255, 128, 64);
        assert_eq!(color.a, 255);
    }

    #[test]
    fn test_rgba_default() {
        let color = Rgba::default();
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);
        assert_eq!(color.a, 255);
    }

    #[test]
    fn test_rgba_to_hsla() {
        let color = Rgba::rgb(255, 0, 0); // Pure red
        let hsla = color.to_hsla();
        assert!((hsla.h - 0.0).abs() < 0.01); // Hue should be 0 (red)
        assert!((hsla.s - 1.0).abs() < 0.01); // Full saturation
        assert!((hsla.l - 0.5).abs() < 0.01); // Middle lightness
    }

    #[test]
    fn test_ansi_colors_default() {
        let ansi = AnsiColors::default();
        assert_eq!(ansi.colors.len(), 16);
    }

    #[test]
    fn test_ansi_colors_get() {
        let ansi = AnsiColors::default();
        assert!(ansi.get(0).is_some());
        assert!(ansi.get(15).is_some());
        assert!(ansi.get(16).is_none());
    }

    #[test]
    fn test_terminal_colors() {
        let theme = CodirigentTheme::dark();
        // #050505
        assert_eq!(theme.terminal_background.r, 5);
        assert_eq!(theme.terminal_background.g, 5);
        assert_eq!(theme.terminal_background.b, 5);
    }

    #[test]
    fn test_hex_to_hsla_valid() {
        // Test 6-digit hex
        let color = hex_to_hsla("#FF0000").unwrap();
        assert!((color.h - 0.0).abs() < 0.01); // Red
        assert!((color.s - 1.0).abs() < 0.01);
        assert!((color.l - 0.5).abs() < 0.01);

        // Test without hash
        let color2 = hex_to_hsla("00FF00").unwrap();
        assert!((color2.h - 0.333).abs() < 0.01); // Green

        // Test 3-digit hex
        let color3 = hex_to_hsla("#F00").unwrap();
        assert!((color3.h - 0.0).abs() < 0.01); // Red
    }

    #[test]
    fn test_hex_to_hsla_invalid() {
        assert!(hex_to_hsla("invalid").is_none());
        assert!(hex_to_hsla("#GGG").is_none());
        assert!(hex_to_hsla("#12345").is_none()); // Wrong length
    }

    #[test]
    fn test_accent_colors() {
        let theme = CodirigentTheme::dark();
        // Primary should be teal-ish (around 174 degrees hue, or ~0.48 normalized)
        assert!(theme.primary.s > 0.4, "Primary should be saturated");
        // Secondary should be blue-ish
        assert!(theme.secondary.s > 0.4, "Secondary should be saturated");
    }

    #[test]
    fn test_session_colors() {
        let theme = CodirigentTheme::dark();
        assert_eq!(theme.session_colors.len(), 6);
        // All session colors should be opaque
        for color in theme.session_colors {
            assert_eq!(color.a, 1.0);
        }
    }

    #[test]
    fn test_typography_values() {
        let theme = CodirigentTheme::dark();
        assert_eq!(theme.font_size_base, 13.0);
        assert!(theme.font_size_small < theme.font_size_base);
        assert!(theme.font_size_large > theme.font_size_base);
    }

    #[test]
    fn test_spacing_values() {
        let theme = CodirigentTheme::dark();
        assert_eq!(theme.spacing_base, 8.0);
        assert!(theme.spacing_small < theme.spacing_base);
        assert!(theme.spacing_large > theme.spacing_base);
    }

    #[test]
    fn test_priority_colors() {
        let theme = CodirigentTheme::dark();
        // Priority colors should all be distinct
        assert_ne!(theme.priority_high, theme.priority_medium);
        assert_ne!(theme.priority_medium, theme.priority_low);
        assert_ne!(theme.priority_high, theme.priority_low);
    }

    #[test]
    fn test_mockup_specific_colors() {
        let dark = CodirigentTheme::dark();
        // All new fields should be opaque or have intentional alpha
        assert!(dark.icon_rail_background.a >= 0.9);
        assert!(dark.drawer_background.a >= 0.9);
        assert!(dark.selected_ring.a >= 0.9);
        assert!(dark.broadcast_accent.a >= 0.9);
        assert!(dark.input_required_accent.a >= 0.9);
        // Semi-transparent fields
        assert!(dark.ai_summary_background.a < 0.5);
        assert!(dark.input_required_background.a < 0.5);
    }

    #[test]
    fn test_light_theme_mockup_colors() {
        let light = CodirigentTheme::light();
        assert!(light.icon_rail_background.a >= 0.9);
        assert!(light.broadcast_accent.a >= 0.9);
        assert!(light.selected_ring.a >= 0.9);
    }
}
