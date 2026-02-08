//! Terminal cell header component.
//!
//! Provides styled headers for terminal cells in the grid, including
//! session name, status indicator, task description, and context usage.

use crate::sidebar::Color;
use codirigent_core::SessionStatus;

/// Terminal header rendering data.
///
/// Contains all the information needed to render a terminal cell header.
#[derive(Debug, Clone)]
pub struct TerminalHeader {
    /// Session name displayed in the header.
    pub session_name: String,
    /// Session group color (for color indicator bar).
    pub session_color: Color,
    /// Current session status.
    pub status: SessionStatus,
    /// Current task description (if any).
    pub task: Option<String>,
    /// Context window usage (0.0 - 1.0).
    pub context_usage: Option<f32>,
    /// Whether this terminal is focused.
    pub is_focused: bool,
    /// Project/repository name (e.g., "backend-repo").
    pub project_name: Option<String>,
    /// CLI engine name (e.g., "Claude", "Gemini 2.0").
    pub cli_name: Option<String>,
    /// Whether the session is waiting for user input.
    pub is_waiting_for_input: bool,
    /// AI-generated summary of current activity.
    pub ai_summary: Option<String>,
    /// Git branch name (if in a git repo).
    pub git_branch: Option<String>,
    /// Git dirty file count (if in a git repo).
    pub git_dirty_count: Option<usize>,
    /// Group name (if session is in a group).
    pub group_name: Option<String>,
}

impl Default for TerminalHeader {
    fn default() -> Self {
        Self {
            session_name: "Session".to_string(),
            session_color: Color::from_hex("#6366f1"),
            status: SessionStatus::Idle,
            task: None,
            context_usage: None,
            is_focused: false,
            project_name: None,
            cli_name: None,
            is_waiting_for_input: false,
            ai_summary: None,
            git_branch: None,
            git_dirty_count: None,
            group_name: None,
        }
    }
}

impl TerminalHeader {
    /// Create a new terminal header.
    pub fn new(name: impl Into<String>, status: SessionStatus) -> Self {
        Self {
            session_name: name.into(),
            status,
            ..Default::default()
        }
    }

    /// Set the session color.
    pub fn with_color(mut self, color: Color) -> Self {
        self.session_color = color;
        self
    }

    /// Set the current task.
    pub fn with_task(mut self, task: impl Into<String>) -> Self {
        self.task = Some(task.into());
        self
    }

    /// Set the context usage.
    pub fn with_context_usage(mut self, usage: f32) -> Self {
        self.context_usage = Some(usage.clamp(0.0, 1.0));
        self
    }

    /// Set whether this terminal is focused.
    pub fn with_focused(mut self, focused: bool) -> Self {
        self.is_focused = focused;
        self
    }

    /// Set the project name.
    pub fn with_project_name(mut self, name: impl Into<String>) -> Self {
        self.project_name = Some(name.into());
        self
    }

    /// Set the CLI engine name.
    pub fn with_cli_name(mut self, name: impl Into<String>) -> Self {
        self.cli_name = Some(name.into());
        self
    }

    /// Set the AI summary.
    pub fn with_ai_summary(mut self, summary: impl Into<String>) -> Self {
        self.ai_summary = Some(summary.into());
        self
    }

    /// Set git branch and dirty count info.
    pub fn with_git_info(mut self, branch: String, dirty: usize) -> Self {
        self.git_branch = Some(branch);
        self.git_dirty_count = Some(dirty);
        self
    }

    /// Set the group name.
    pub fn with_group_name(mut self, name: impl Into<String>) -> Self {
        self.group_name = Some(name.into());
        self
    }

    /// Set whether waiting for input.
    pub fn set_waiting_for_input(&mut self, waiting: bool) {
        self.is_waiting_for_input = waiting;
    }

    /// Get the status indicator info.
    pub fn status_indicator(&self) -> StatusIndicator {
        StatusIndicator::for_status(self.status)
    }

    /// Get the context usage display info.
    pub fn context_display(&self) -> Option<ContextDisplay> {
        self.context_usage.map(ContextDisplay::new)
    }

    /// Get the task badge info.
    pub fn task_badge(&self) -> Option<TaskBadge> {
        self.task.as_ref().map(|t| TaskBadge::new(t))
    }
}

/// Status indicator display info.
#[derive(Debug, Clone, Copy)]
pub struct StatusIndicator {
    /// Status text label.
    pub text: &'static str,
    /// Indicator color.
    pub color: Color,
    /// Whether to show animated dot.
    pub animated: bool,
}

impl StatusIndicator {
    /// Get status indicator for a session status.
    pub fn for_status(status: SessionStatus) -> Self {
        match status {
            SessionStatus::Idle => Self {
                text: "Idle",
                color: Color::from_hex("#52525b"),
                animated: false,
            },
            SessionStatus::Working => Self {
                text: "Working",
                color: Color::from_hex("#f59e0b"),
                animated: true,
            },
            SessionStatus::WaitingForInput => Self {
                text: "Waiting",
                color: Color::from_hex("#f43f5e"),
                animated: true,
            },
            SessionStatus::Done => Self {
                text: "Done",
                color: Color::from_hex("#10b981"),
                animated: false,
            },
            SessionStatus::Error => Self {
                text: "Error",
                color: Color::from_hex("#ef4444"),
                animated: false,
            },
        }
    }
}

/// Context usage display info.
#[derive(Debug, Clone, Copy)]
pub struct ContextDisplay {
    /// Usage percentage (0.0 - 1.0).
    pub percentage: f32,
    /// Display text (e.g., "85%").
    formatted: [u8; 8],
    /// Length of formatted string.
    formatted_len: usize,
    /// Color based on usage level.
    pub color: Color,
    /// Usage level for styling.
    pub level: ContextLevel,
}

impl ContextDisplay {
    /// Create a new context display.
    pub fn new(percentage: f32) -> Self {
        let percentage = percentage.clamp(0.0, 1.0);
        let level = ContextLevel::from_percentage(percentage);
        let color = level.color();

        // Format percentage
        let pct = (percentage * 100.0).round() as u32;
        let mut formatted = [0u8; 8];
        let s = format!("{}%", pct);
        let bytes = s.as_bytes();
        let len = bytes.len().min(8);
        formatted[..len].copy_from_slice(&bytes[..len]);

        Self {
            percentage,
            formatted,
            formatted_len: len,
            color,
            level,
        }
    }

    /// Get the formatted percentage string.
    pub fn text(&self) -> &str {
        std::str::from_utf8(&self.formatted[..self.formatted_len]).unwrap_or("0%")
    }
}

/// Context usage level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextLevel {
    /// Normal usage (< 70%).
    Normal,
    /// Warning usage (70-90%).
    Warning,
    /// Critical usage (> 90%).
    Critical,
}

impl ContextLevel {
    /// Get level from percentage.
    pub fn from_percentage(pct: f32) -> Self {
        if pct >= 0.9 {
            Self::Critical
        } else if pct >= 0.7 {
            Self::Warning
        } else {
            Self::Normal
        }
    }

    /// Get the color for this level.
    pub fn color(&self) -> Color {
        match self {
            Self::Normal => Color::from_hex("#888888"),
            Self::Warning => Color::from_hex("#F59E0B"),
            Self::Critical => Color::from_hex("#FF6B6B"),
        }
    }
}

/// Task badge display info.
#[derive(Debug, Clone)]
pub struct TaskBadge {
    /// Task description text.
    pub text: String,
    /// Truncated text for display (if too long).
    pub display_text: String,
    /// Badge background color.
    pub bg_color: Color,
    /// Badge text color.
    pub text_color: Color,
}

impl TaskBadge {
    /// Maximum characters to display.
    const MAX_DISPLAY_LEN: usize = 30;

    /// Create a new task badge.
    ///
    /// Long task descriptions are truncated to `MAX_DISPLAY_LEN` characters
    /// with an ellipsis. Uses character-aware truncation to handle UTF-8 safely.
    pub fn new(text: &str) -> Self {
        let char_count = text.chars().count();
        let display_text = if char_count > Self::MAX_DISPLAY_LEN {
            let truncated: String = text.chars().take(Self::MAX_DISPLAY_LEN - 3).collect();
            format!("{}...", truncated)
        } else {
            text.to_string()
        };

        Self {
            text: text.to_string(),
            display_text,
            bg_color: Color::rgba(0.31, 0.55, 0.94, 0.15), // Blue @ 15%
            text_color: Color::from_hex("#5B8DEF"),
        }
    }
}

/// Rendering hints for the terminal header.
#[derive(Debug, Clone)]
pub struct TerminalHeaderRenderHints {
    /// Session name.
    pub name: String,
    /// Color indicator color.
    pub color_indicator: Color,
    /// Status indicator.
    pub status: StatusIndicator,
    /// Task badge (if any).
    pub task: Option<TaskBadge>,
    /// Context display (if any).
    pub context: Option<ContextDisplay>,
    /// Whether focused (for border highlight).
    pub is_focused: bool,
    /// Header height in pixels.
    pub height: f32,
    /// Project/repository name.
    pub project_name: Option<String>,
    /// CLI engine name.
    pub cli_name: Option<String>,
    /// Whether waiting for user input.
    pub is_waiting_for_input: bool,
    /// AI-generated summary.
    pub ai_summary: Option<String>,
    /// Git branch name.
    pub git_branch: Option<String>,
    /// Git dirty file count.
    pub git_dirty_count: Option<usize>,
    /// Group name (if session is in a group).
    pub group_name: Option<String>,
}

impl TerminalHeader {
    /// Default header height.
    pub const DEFAULT_HEIGHT: f32 = 32.0;

    /// Generate rendering hints.
    pub fn render_hints(&self) -> TerminalHeaderRenderHints {
        TerminalHeaderRenderHints {
            name: self.session_name.clone(),
            color_indicator: self.session_color,
            status: self.status_indicator(),
            task: self.task_badge(),
            context: self.context_display(),
            is_focused: self.is_focused,
            height: Self::DEFAULT_HEIGHT,
            project_name: self.project_name.clone(),
            cli_name: self.cli_name.clone(),
            is_waiting_for_input: self.is_waiting_for_input,
            ai_summary: self.ai_summary.clone(),
            git_branch: self.git_branch.clone(),
            git_dirty_count: self.git_dirty_count,
            group_name: self.group_name.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_header_new() {
        let header = TerminalHeader::new("Test Session", SessionStatus::Working);
        assert_eq!(header.session_name, "Test Session");
        assert_eq!(header.status, SessionStatus::Working);
        assert!(header.task.is_none());
        assert!(header.context_usage.is_none());
    }

    #[test]
    fn test_terminal_header_default() {
        let header = TerminalHeader::default();
        assert_eq!(header.session_name, "Session");
        assert_eq!(header.status, SessionStatus::Idle);
    }

    #[test]
    fn test_terminal_header_builder() {
        let header = TerminalHeader::new("S1", SessionStatus::Working)
            .with_color(Color::from_hex("#FF0000"))
            .with_task("Implementing feature")
            .with_context_usage(0.75)
            .with_focused(true);

        assert_eq!(header.session_name, "S1");
        assert!(header.session_color.r > 0.9);
        assert_eq!(header.task, Some("Implementing feature".to_string()));
        assert_eq!(header.context_usage, Some(0.75));
        assert!(header.is_focused);
    }

    #[test]
    fn test_status_indicator_idle() {
        let indicator = StatusIndicator::for_status(SessionStatus::Idle);
        assert_eq!(indicator.text, "Idle");
        assert!(!indicator.animated);
    }

    #[test]
    fn test_status_indicator_working() {
        let indicator = StatusIndicator::for_status(SessionStatus::Working);
        assert_eq!(indicator.text, "Working");
        assert!(indicator.animated);
    }

    #[test]
    fn test_status_indicator_waiting() {
        let indicator = StatusIndicator::for_status(SessionStatus::WaitingForInput);
        assert_eq!(indicator.text, "Waiting");
        assert!(indicator.animated);
    }

    #[test]
    fn test_status_indicator_done() {
        let indicator = StatusIndicator::for_status(SessionStatus::Done);
        assert_eq!(indicator.text, "Done");
        assert!(!indicator.animated);
    }

    #[test]
    fn test_status_indicator_error() {
        let indicator = StatusIndicator::for_status(SessionStatus::Error);
        assert_eq!(indicator.text, "Error");
        assert!(!indicator.animated);
    }

    #[test]
    fn test_context_display_normal() {
        let display = ContextDisplay::new(0.5);
        assert_eq!(display.text(), "50%");
        assert_eq!(display.level, ContextLevel::Normal);
    }

    #[test]
    fn test_context_display_warning() {
        let display = ContextDisplay::new(0.75);
        assert_eq!(display.text(), "75%");
        assert_eq!(display.level, ContextLevel::Warning);
    }

    #[test]
    fn test_context_display_critical() {
        let display = ContextDisplay::new(0.95);
        assert_eq!(display.text(), "95%");
        assert_eq!(display.level, ContextLevel::Critical);
    }

    #[test]
    fn test_context_display_clamping() {
        let display = ContextDisplay::new(1.5);
        assert_eq!(display.percentage, 1.0);
        assert_eq!(display.text(), "100%");

        let display2 = ContextDisplay::new(-0.5);
        assert_eq!(display2.percentage, 0.0);
        assert_eq!(display2.text(), "0%");
    }

    #[test]
    fn test_context_level_boundaries() {
        assert_eq!(ContextLevel::from_percentage(0.69), ContextLevel::Normal);
        assert_eq!(ContextLevel::from_percentage(0.7), ContextLevel::Warning);
        assert_eq!(ContextLevel::from_percentage(0.89), ContextLevel::Warning);
        assert_eq!(ContextLevel::from_percentage(0.9), ContextLevel::Critical);
    }

    #[test]
    fn test_context_level_colors_distinct() {
        let normal = ContextLevel::Normal.color();
        let warning = ContextLevel::Warning.color();
        let critical = ContextLevel::Critical.color();

        assert_ne!(normal.r, warning.r);
        assert_ne!(warning.r, critical.r);
    }

    #[test]
    fn test_task_badge_short() {
        let badge = TaskBadge::new("Fix bug");
        assert_eq!(badge.text, "Fix bug");
        assert_eq!(badge.display_text, "Fix bug");
    }

    #[test]
    fn test_task_badge_long_truncation() {
        let long_text = "This is a very long task description that should be truncated";
        let badge = TaskBadge::new(long_text);
        assert!(badge.display_text.chars().count() <= TaskBadge::MAX_DISPLAY_LEN);
        assert!(badge.display_text.ends_with("..."));
    }

    #[test]
    fn test_task_badge_utf8_truncation() {
        // Test with multi-byte UTF-8 characters (Chinese) - must be longer than MAX_DISPLAY_LEN (30)
        let chinese_text =
            "這是一個非常非常非常非常非常非常非常非常長的任務描述，需要被截斷才能顯示在介面上";
        assert!(chinese_text.chars().count() > TaskBadge::MAX_DISPLAY_LEN);
        let badge = TaskBadge::new(chinese_text);
        // Should not panic and should be valid UTF-8
        assert!(badge.display_text.chars().count() <= TaskBadge::MAX_DISPLAY_LEN);
        assert!(badge.display_text.ends_with("..."));
        // Full text should be preserved
        assert_eq!(badge.text, chinese_text);
    }

    #[test]
    fn test_task_badge_emoji_truncation() {
        // Test with emoji characters
        let emoji_text = "🚀🎉🔥⭐💻🎯🌟✨📝🔧💡🎨🎸🎤🎬🎮🎯🎲🎪🎭";
        let badge = TaskBadge::new(emoji_text);
        // Should not panic and should be valid UTF-8
        assert!(badge.display_text.chars().count() <= TaskBadge::MAX_DISPLAY_LEN);
    }

    #[test]
    fn test_render_hints() {
        let header = TerminalHeader::new("Test", SessionStatus::Working)
            .with_task("Task")
            .with_context_usage(0.8)
            .with_focused(true);

        let hints = header.render_hints();
        assert_eq!(hints.name, "Test");
        assert!(hints.task.is_some());
        assert!(hints.context.is_some());
        assert!(hints.is_focused);
        assert_eq!(hints.height, TerminalHeader::DEFAULT_HEIGHT);
    }

    #[test]
    fn test_render_hints_minimal() {
        let header = TerminalHeader::new("S1", SessionStatus::Idle);
        let hints = header.render_hints();
        assert!(hints.task.is_none());
        assert!(hints.context.is_none());
        assert!(!hints.is_focused);
    }

    #[test]
    fn test_header_status_indicator() {
        let header = TerminalHeader::new("S1", SessionStatus::Working);
        let indicator = header.status_indicator();
        assert_eq!(indicator.text, "Working");
    }

    #[test]
    fn test_header_context_display() {
        let header = TerminalHeader::new("S1", SessionStatus::Idle).with_context_usage(0.5);
        let context = header.context_display();
        assert!(context.is_some());
        assert_eq!(context.unwrap().text(), "50%");
    }

    #[test]
    fn test_header_task_badge() {
        let header = TerminalHeader::new("S1", SessionStatus::Idle).with_task("Testing");
        let badge = header.task_badge();
        assert!(badge.is_some());
        assert_eq!(badge.unwrap().text, "Testing");
    }

    #[test]
    fn test_terminal_header_project_name() {
        let header =
            TerminalHeader::new("S1", SessionStatus::Working).with_project_name("backend-repo");
        assert_eq!(header.project_name, Some("backend-repo".to_string()));
        let hints = header.render_hints();
        assert_eq!(hints.project_name, Some("backend-repo".to_string()));
    }

    #[test]
    fn test_terminal_header_cli_name() {
        let header = TerminalHeader::new("S1", SessionStatus::Working).with_cli_name("Gemini 2.0");
        assert_eq!(header.cli_name, Some("Gemini 2.0".to_string()));
    }

    #[test]
    fn test_terminal_header_ai_summary() {
        let header = TerminalHeader::new("S1", SessionStatus::Working)
            .with_ai_summary("Implementing JWT service");
        assert_eq!(
            header.ai_summary,
            Some("Implementing JWT service".to_string())
        );
    }

    #[test]
    fn test_terminal_header_waiting_for_input() {
        let mut header = TerminalHeader::new("S1", SessionStatus::WaitingForInput);
        assert!(!header.is_waiting_for_input);
        header.set_waiting_for_input(true);
        assert!(header.is_waiting_for_input);
        let hints = header.render_hints();
        assert!(hints.is_waiting_for_input);
    }

    #[test]
    fn test_terminal_header_full_builder() {
        let header = TerminalHeader::new("Auth Refactor", SessionStatus::Working)
            .with_color(Color::from_hex("#6366f1"))
            .with_task("Refactor auth module")
            .with_context_usage(0.72)
            .with_focused(true)
            .with_project_name("backend-repo")
            .with_cli_name("Gemini 2.0")
            .with_ai_summary("Implementing JWT...");

        let hints = header.render_hints();
        assert_eq!(hints.name, "Auth Refactor");
        assert_eq!(hints.project_name, Some("backend-repo".to_string()));
        assert_eq!(hints.cli_name, Some("Gemini 2.0".to_string()));
        assert_eq!(hints.ai_summary, Some("Implementing JWT...".to_string()));
        assert!(hints.is_focused);
    }
}
