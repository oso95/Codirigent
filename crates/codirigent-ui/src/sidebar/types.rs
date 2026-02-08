//! Sidebar types and data structures.

use codirigent_core::{SessionId, SessionStatus};

/// Events emitted by the sidebar.
#[derive(Debug, Clone, PartialEq)]
pub enum SidebarEvent {
    /// Request to focus a specific session.
    FocusSession(SessionId),
    /// Request to create a new session.
    NewSession,
    /// Request to rename a session.
    RenameSession {
        /// Session to rename.
        id: SessionId,
        /// New name for the session.
        new_name: String,
    },
    /// Request to close a session.
    CloseSession(SessionId),
    /// Toggle group expansion.
    ToggleGroup(String),
}

/// Session group for visual organization.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionGroup {
    /// Group name.
    pub name: String,
    /// Group color (hex format, e.g., "#FF5733").
    pub color: String,
    /// Whether the group is expanded.
    pub expanded: bool,
}

impl Default for SessionGroup {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            color: "#6c7086".to_string(),
            expanded: true,
        }
    }
}

impl SessionGroup {
    /// Create a new session group.
    pub fn new(name: String, color: String) -> Self {
        Self {
            name,
            color,
            expanded: true,
        }
    }

    /// Toggle the expanded state of the group.
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }
}

/// RGBA color representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red component (0.0 - 1.0).
    pub r: f32,
    /// Green component (0.0 - 1.0).
    pub g: f32,
    /// Blue component (0.0 - 1.0).
    pub b: f32,
    /// Alpha component (0.0 - 1.0).
    pub a: f32,
}

impl Color {
    /// Create a new color from RGBA components.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Parse a hex color string to Color.
    ///
    /// Supports formats: "#RRGGBB" or "RRGGBB".
    pub fn from_hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return Self::rgba(0.5, 0.5, 0.5, 1.0);
        }

        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128) as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128) as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128) as f32 / 255.0;

        Self::rgba(r, g, b, 1.0)
    }

    /// Convert to GPUI Hsla color.
    ///
    /// This allows using sidebar colors directly in GPUI rendering.
    #[cfg(feature = "gpui-full")]
    pub fn to_hsla(&self) -> gpui::Hsla {
        gpui::Rgba {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
        .into()
    }
}

#[cfg(feature = "gpui-full")]
impl From<Color> for gpui::Hsla {
    fn from(color: Color) -> Self {
        color.to_hsla()
    }
}

/// Color mapping for session status indicators.
#[derive(Debug, Clone)]
pub struct StatusColors {
    /// Color for Idle status.
    pub idle: Color,
    /// Color for Working status.
    pub working: Color,
    /// Color for WaitingForInput status.
    pub waiting_for_input: Color,
    /// Color for Done status.
    pub done: Color,
    /// Color for Error status.
    pub error: Color,
}

impl Default for StatusColors {
    fn default() -> Self {
        Self {
            idle: Color::from_hex("#6c7086"),              // Gray
            working: Color::from_hex("#f9e2af"),           // Yellow
            waiting_for_input: Color::from_hex("#fab387"), // Orange
            done: Color::from_hex("#a6e3a1"),              // Green
            error: Color::from_hex("#f38ba8"),             // Red
        }
    }
}

impl StatusColors {
    /// Get the color for a given session status.
    pub fn color_for(&self, status: SessionStatus) -> Color {
        match status {
            SessionStatus::Idle => self.idle,
            SessionStatus::Working => self.working,
            SessionStatus::WaitingForInput => self.waiting_for_input,
            SessionStatus::Done => self.done,
            SessionStatus::Error => self.error,
        }
    }
}

/// Rendering hints for the sidebar.
///
/// This struct provides layout information for GPUI rendering.
#[derive(Debug, Clone)]
pub struct SidebarRenderHints {
    /// Session items to render (in order).
    pub items: Vec<SidebarItem>,
    /// Total height needed for all items.
    pub total_height: f32,
    /// Sidebar width.
    pub width: f32,
}

/// A single item in the sidebar (session or group header).
#[derive(Debug, Clone)]
pub enum SidebarItem {
    /// A group header.
    GroupHeader {
        /// Group name.
        name: String,
        /// Group color.
        color: Color,
        /// Whether the group is expanded.
        expanded: bool,
        /// Number of sessions in the group.
        session_count: usize,
    },
    /// A session item.
    Session {
        /// Session ID.
        id: SessionId,
        /// Session name.
        name: String,
        /// Session status.
        status: SessionStatus,
        /// Status indicator color.
        status_color: Color,
        /// Whether this session is focused.
        is_focused: bool,
        /// Whether this session is being renamed.
        is_editing: bool,
        /// Indentation level (0 for ungrouped, 1 for grouped).
        indent_level: u8,
        /// Current task description (if any).
        task: Option<String>,
        /// Context window usage percentage (0.0 - 1.0).
        context_usage: Option<f32>,
        /// Session group color indicator.
        group_color: Option<Color>,
    },
}

/// Status badge styling information.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusBadge {
    /// Badge text.
    pub text: &'static str,
    /// Badge background color.
    pub bg_color: Color,
    /// Badge text color.
    pub text_color: Color,
    /// Whether the badge should show an animated dot.
    pub animated: bool,
}

impl StatusBadge {
    /// Get the badge for a session status with theme colors.
    pub fn for_status(status: SessionStatus, status_colors: &StatusColors) -> Self {
        match status {
            SessionStatus::Idle => Self {
                text: "Idle",
                bg_color: Color::rgba(0.4, 0.4, 0.4, 0.2),
                text_color: status_colors.idle,
                animated: false,
            },
            SessionStatus::Working => Self {
                text: "Working",
                bg_color: Color::rgba(0.31, 0.80, 0.77, 0.15), // Teal @ 15%
                text_color: status_colors.working,
                animated: true,
            },
            SessionStatus::WaitingForInput => Self {
                text: "Waiting",
                bg_color: Color::rgba(1.0, 0.42, 0.42, 0.15), // Red @ 15%
                text_color: status_colors.waiting_for_input,
                animated: true,
            },
            SessionStatus::Done => Self {
                text: "Done",
                bg_color: Color::rgba(0.31, 0.80, 0.77, 0.15), // Teal @ 15%
                text_color: status_colors.done,
                animated: false,
            },
            SessionStatus::Error => Self {
                text: "Error",
                bg_color: Color::rgba(1.0, 0.42, 0.42, 0.15), // Red @ 15%
                text_color: status_colors.error,
                animated: false,
            },
        }
    }
}

/// Context usage level for color coding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextUsageLevel {
    /// Normal usage (< 70%).
    Normal,
    /// Warning usage (70-90%).
    Warning,
    /// Critical usage (> 90%).
    Critical,
}

impl ContextUsageLevel {
    /// Determine the usage level from a percentage.
    pub fn from_percentage(pct: f32) -> Self {
        if pct >= 0.9 {
            Self::Critical
        } else if pct >= 0.7 {
            Self::Warning
        } else {
            Self::Normal
        }
    }

    /// Get the color for this usage level.
    pub fn color(&self) -> Color {
        match self {
            Self::Normal => Color::from_hex("#888888"), // Secondary text
            Self::Warning => Color::from_hex("#F59E0B"), // Orange
            Self::Critical => Color::from_hex("#FF6B6B"), // Red
        }
    }
}
