//! Status bar component.
//!
//! Displays session counts, task queue status, and version information
//! at the bottom of the workspace.

use crate::sidebar::Color;

/// Status bar component state.
#[derive(Debug)]
pub struct StatusBar {
    /// Total session count.
    total_sessions: usize,
    /// Working session count.
    working_sessions: usize,
    /// Waiting for input session count.
    waiting_sessions: usize,
    /// Tasks in queue count.
    tasks_in_queue: usize,
    /// Tasks in progress count.
    tasks_in_progress: usize,
    /// Application version string.
    version: String,
    /// Height in pixels.
    height: f32,
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusBar {
    /// Default status bar height.
    pub const DEFAULT_HEIGHT: f32 = 24.0;

    /// Create a new status bar.
    pub fn new() -> Self {
        Self {
            total_sessions: 0,
            working_sessions: 0,
            waiting_sessions: 0,
            tasks_in_queue: 0,
            tasks_in_progress: 0,
            version: env!("CARGO_PKG_VERSION").to_string(),
            height: Self::DEFAULT_HEIGHT,
        }
    }

    /// Create with a specific version.
    pub fn with_version(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            ..Self::new()
        }
    }

    /// Update session counts.
    pub fn set_session_counts(&mut self, total: usize, working: usize, waiting: usize) {
        self.total_sessions = total;
        self.working_sessions = working;
        self.waiting_sessions = waiting;
    }

    /// Update task counts.
    pub fn set_task_counts(&mut self, in_queue: usize, in_progress: usize) {
        self.tasks_in_queue = in_queue;
        self.tasks_in_progress = in_progress;
    }

    /// Get total session count.
    pub fn total_sessions(&self) -> usize {
        self.total_sessions
    }

    /// Get working session count.
    pub fn working_sessions(&self) -> usize {
        self.working_sessions
    }

    /// Get waiting session count.
    pub fn waiting_sessions(&self) -> usize {
        self.waiting_sessions
    }

    /// Get tasks in queue.
    pub fn tasks_in_queue(&self) -> usize {
        self.tasks_in_queue
    }

    /// Get tasks in progress.
    pub fn tasks_in_progress(&self) -> usize {
        self.tasks_in_progress
    }

    /// Get the version string.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Get the status bar height.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Set the status bar height.
    pub fn set_height(&mut self, height: f32) {
        self.height = height.max(16.0);
    }

    /// Generate left section items.
    pub fn left_section(&self) -> Vec<StatusBarItem> {
        vec![
            StatusBarItem::SessionCount {
                total: self.total_sessions,
                color: Color::from_hex("#39d353"),
            },
            StatusBarItem::SessionStatus {
                label: "Working",
                count: self.working_sessions,
                color: Color::from_hex("#39d353"),
            },
            StatusBarItem::SessionStatus {
                label: "Waiting",
                count: self.waiting_sessions,
                color: Color::from_hex("#FF6B6B"),
            },
        ]
    }

    /// Generate right section items.
    pub fn right_section(&self) -> Vec<StatusBarItem> {
        vec![
            StatusBarItem::TaskQueue {
                in_queue: self.tasks_in_queue,
                in_progress: self.tasks_in_progress,
            },
            StatusBarItem::Version(self.version.clone()),
        ]
    }
}

/// Status bar item types.
#[derive(Debug, Clone)]
pub enum StatusBarItem {
    /// Session count with indicator dot.
    SessionCount {
        /// Total session count.
        total: usize,
        /// Indicator color.
        color: Color,
    },
    /// Session status count (working/waiting).
    SessionStatus {
        /// Status label.
        label: &'static str,
        /// Count.
        count: usize,
        /// Status color.
        color: Color,
    },
    /// Task queue status.
    TaskQueue {
        /// Tasks waiting in queue.
        in_queue: usize,
        /// Tasks currently in progress.
        in_progress: usize,
    },
    /// Version display.
    Version(String),
    /// Separator.
    Separator,
}

impl StatusBarItem {
    /// Get the text representation of this item.
    pub fn text(&self) -> String {
        match self {
            Self::SessionCount { total, .. } => format!("{} sessions", total),
            Self::SessionStatus { label, count, .. } => format!("{}: {}", label, count),
            Self::TaskQueue {
                in_queue,
                in_progress,
            } => {
                if *in_queue == 0 && *in_progress == 0 {
                    "No tasks".to_string()
                } else {
                    format!("Tasks: {} queued, {} active", in_queue, in_progress)
                }
            }
            Self::Version(v) => format!("v{}", v),
            Self::Separator => "│".to_string(),
        }
    }
}

/// Rendering hints for the status bar.
#[derive(Debug, Clone)]
pub struct StatusBarRenderHints {
    /// Left section items.
    pub left: Vec<StatusBarItem>,
    /// Right section items.
    pub right: Vec<StatusBarItem>,
    /// Bar height.
    pub height: f32,
    /// Background color.
    pub background: Color,
    /// Text color.
    pub text_color: Color,
}

impl StatusBar {
    /// Generate rendering hints.
    pub fn render_hints(&self) -> StatusBarRenderHints {
        StatusBarRenderHints {
            left: self.left_section(),
            right: self.right_section(),
            height: self.height,
            background: Color::from_hex("#0a0a0c"),
            text_color: Color::from_hex("#888888"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_new() {
        let bar = StatusBar::new();
        assert_eq!(bar.total_sessions(), 0);
        assert_eq!(bar.working_sessions(), 0);
        assert_eq!(bar.waiting_sessions(), 0);
        assert_eq!(bar.height(), StatusBar::DEFAULT_HEIGHT);
    }

    #[test]
    fn test_status_bar_default() {
        let bar = StatusBar::default();
        assert_eq!(bar.total_sessions(), 0);
    }

    #[test]
    fn test_with_version() {
        let bar = StatusBar::with_version("1.2.3");
        assert_eq!(bar.version(), "1.2.3");
    }

    #[test]
    fn test_set_session_counts() {
        let mut bar = StatusBar::new();
        bar.set_session_counts(5, 3, 1);
        assert_eq!(bar.total_sessions(), 5);
        assert_eq!(bar.working_sessions(), 3);
        assert_eq!(bar.waiting_sessions(), 1);
    }

    #[test]
    fn test_set_task_counts() {
        let mut bar = StatusBar::new();
        bar.set_task_counts(10, 2);
        assert_eq!(bar.tasks_in_queue(), 10);
        assert_eq!(bar.tasks_in_progress(), 2);
    }

    #[test]
    fn test_set_height() {
        let mut bar = StatusBar::new();
        bar.set_height(30.0);
        assert_eq!(bar.height(), 30.0);

        // Minimum enforced
        bar.set_height(5.0);
        assert!(bar.height() >= 16.0);
    }

    #[test]
    fn test_left_section() {
        let mut bar = StatusBar::new();
        bar.set_session_counts(4, 2, 1);

        let left = bar.left_section();
        assert_eq!(left.len(), 3);

        // First item should be total sessions
        if let StatusBarItem::SessionCount { total, .. } = &left[0] {
            assert_eq!(*total, 4);
        } else {
            panic!("Expected SessionCount");
        }
    }

    #[test]
    fn test_right_section() {
        let mut bar = StatusBar::new();
        bar.set_task_counts(5, 2);

        let right = bar.right_section();
        assert_eq!(right.len(), 2);

        // First should be task queue
        if let StatusBarItem::TaskQueue {
            in_queue,
            in_progress,
        } = &right[0]
        {
            assert_eq!(*in_queue, 5);
            assert_eq!(*in_progress, 2);
        } else {
            panic!("Expected TaskQueue");
        }

        // Second should be version
        assert!(matches!(&right[1], StatusBarItem::Version(_)));
    }

    #[test]
    fn test_session_count_text() {
        let item = StatusBarItem::SessionCount {
            total: 5,
            color: Color::from_hex("#000"),
        };
        assert_eq!(item.text(), "5 sessions");
    }

    #[test]
    fn test_session_status_text() {
        let item = StatusBarItem::SessionStatus {
            label: "Working",
            count: 3,
            color: Color::from_hex("#000"),
        };
        assert_eq!(item.text(), "Working: 3");
    }

    #[test]
    fn test_task_queue_text() {
        let item = StatusBarItem::TaskQueue {
            in_queue: 5,
            in_progress: 2,
        };
        assert_eq!(item.text(), "Tasks: 5 queued, 2 active");
    }

    #[test]
    fn test_task_queue_text_empty() {
        let item = StatusBarItem::TaskQueue {
            in_queue: 0,
            in_progress: 0,
        };
        assert_eq!(item.text(), "No tasks");
    }

    #[test]
    fn test_version_text() {
        let item = StatusBarItem::Version("1.0.0".to_string());
        assert_eq!(item.text(), "v1.0.0");
    }

    #[test]
    fn test_separator_text() {
        let item = StatusBarItem::Separator;
        assert_eq!(item.text(), "│");
    }

    #[test]
    fn test_render_hints() {
        let mut bar = StatusBar::new();
        bar.set_session_counts(4, 2, 1);
        bar.set_task_counts(3, 1);

        let hints = bar.render_hints();
        assert!(!hints.left.is_empty());
        assert!(!hints.right.is_empty());
        assert_eq!(hints.height, StatusBar::DEFAULT_HEIGHT);
    }

    #[test]
    fn test_version_from_cargo() {
        let bar = StatusBar::new();
        // Should contain the actual crate version
        assert!(!bar.version().is_empty());
    }
}
