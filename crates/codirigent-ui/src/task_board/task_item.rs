//! Task item component for the task board.

use crate::sidebar::Color;

/// Task priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TaskPriority {
    /// High priority (urgent).
    High,
    /// Medium priority (normal).
    #[default]
    Medium,
    /// Low priority (can wait).
    Low,
}

impl TaskPriority {
    /// Get the indicator color for this priority.
    pub fn color(&self) -> Color {
        match self {
            Self::High => Color::from_hex("#FF6B6B"),   // Red
            Self::Medium => Color::from_hex("#F59E0B"), // Orange
            Self::Low => Color::from_hex("#5B8DEF"),    // Blue
        }
    }

    /// Get the display label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
        }
    }
}

/// Task status in the workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TaskStatus {
    /// Waiting in queue.
    #[default]
    Queued,
    /// Currently being worked on.
    InProgress,
    /// Waiting for review.
    PendingReview,
    /// Completed.
    Completed,
}

impl TaskStatus {
    /// Get the display label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::InProgress => "In Progress",
            Self::PendingReview => "Pending Review",
            Self::Completed => "Completed",
        }
    }

    /// Get the status badge color.
    pub fn badge_color(&self) -> Color {
        match self {
            Self::Queued => Color::from_hex("#666666"),
            Self::InProgress => Color::from_hex("#4ECDC4"),
            Self::PendingReview => Color::from_hex("#F59E0B"),
            Self::Completed => Color::from_hex("#4ECDC4"),
        }
    }
}

/// A tag attached to a task.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskTag {
    /// Tag text.
    pub text: String,
    /// Tag color.
    pub color: Color,
}

impl TaskTag {
    /// Create a new tag.
    pub fn new(text: impl Into<String>, color: Color) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }

    /// Create a tag with default color.
    pub fn simple(text: impl Into<String>) -> Self {
        Self::new(text, Color::from_hex("#5B8DEF"))
    }
}

/// A task item in the task board.
#[derive(Debug, Clone, Default)]
pub struct TaskItem {
    /// Unique task identifier.
    pub id: String,
    /// Task title.
    pub title: String,
    /// Task priority.
    pub priority: TaskPriority,
    /// Task status.
    pub status: TaskStatus,
    /// Task tags.
    pub tags: Vec<TaskTag>,
    /// Assigned session name (if any).
    pub assigned_to: Option<String>,
    /// Whether this task is selected.
    pub is_selected: bool,
    /// Whether this task is hovered.
    pub is_hovered: bool,
    /// Estimated time (for display).
    pub estimated_time: Option<String>,
    /// Created timestamp.
    pub created_at: Option<String>,
}

impl TaskItem {
    /// Create a new task item.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            ..Default::default()
        }
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the status.
    pub fn with_status(mut self, status: TaskStatus) -> Self {
        self.status = status;
        self
    }

    /// Add a tag.
    pub fn with_tag(mut self, tag: TaskTag) -> Self {
        self.tags.push(tag);
        self
    }

    /// Add a simple tag.
    pub fn with_simple_tag(mut self, text: impl Into<String>) -> Self {
        self.tags.push(TaskTag::simple(text));
        self
    }

    /// Set assigned session.
    pub fn with_assigned_to(mut self, session: impl Into<String>) -> Self {
        self.assigned_to = Some(session.into());
        self
    }

    /// Set estimated time.
    pub fn with_estimated_time(mut self, time: impl Into<String>) -> Self {
        self.estimated_time = Some(time.into());
        self
    }

    /// Set created timestamp.
    pub fn with_created_at(mut self, timestamp: impl Into<String>) -> Self {
        self.created_at = Some(timestamp.into());
        self
    }

    /// Get the priority indicator.
    pub fn priority_indicator(&self) -> PriorityIndicator {
        PriorityIndicator {
            priority: self.priority,
            color: self.priority.color(),
        }
    }

    /// Get the status badge.
    pub fn status_badge(&self) -> StatusBadge {
        StatusBadge {
            status: self.status,
            label: self.status.label(),
            color: self.status.badge_color(),
        }
    }

    /// Get available actions for this task.
    pub fn available_actions(&self) -> Vec<TaskItemAction> {
        match self.status {
            TaskStatus::Queued => vec![
                TaskItemAction::Assign,
                TaskItemAction::Edit,
                TaskItemAction::Delete,
            ],
            TaskStatus::InProgress => vec![TaskItemAction::MarkForReview, TaskItemAction::Edit],
            TaskStatus::PendingReview => vec![TaskItemAction::Approve, TaskItemAction::Reject],
            TaskStatus::Completed => vec![TaskItemAction::Reopen, TaskItemAction::Delete],
        }
    }
}

/// Priority indicator for rendering.
#[derive(Debug, Clone, Copy)]
pub struct PriorityIndicator {
    /// The priority level.
    pub priority: TaskPriority,
    /// Indicator color.
    pub color: Color,
}

/// Status badge for rendering.
#[derive(Debug, Clone, Copy)]
pub struct StatusBadge {
    /// The status.
    pub status: TaskStatus,
    /// Badge label.
    pub label: &'static str,
    /// Badge color.
    pub color: Color,
}

/// Actions available on a task item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskItemAction {
    /// Assign to a session.
    Assign,
    /// Edit the task.
    Edit,
    /// Delete the task.
    Delete,
    /// Mark task for review.
    MarkForReview,
    /// Approve reviewed task.
    Approve,
    /// Reject reviewed task (back to in progress).
    Reject,
    /// Reopen completed task.
    Reopen,
}

impl TaskItemAction {
    /// Get the display label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Assign => "Assign",
            Self::Edit => "Edit",
            Self::Delete => "Delete",
            Self::MarkForReview => "Review",
            Self::Approve => "Approve",
            Self::Reject => "Reject",
            Self::Reopen => "Reopen",
        }
    }

    /// Get the action icon.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Assign => "+",
            Self::Edit => "E",
            Self::Delete => "X",
            Self::MarkForReview => "R",
            Self::Approve => "OK",
            Self::Reject => "NO",
            Self::Reopen => "RE",
        }
    }
}

/// Rendering hints for a task item.
#[derive(Debug, Clone)]
pub struct TaskItemRenderHints {
    /// Task ID.
    pub id: String,
    /// Task title.
    pub title: String,
    /// Priority indicator.
    pub priority: PriorityIndicator,
    /// Status badge.
    pub status: StatusBadge,
    /// Task tags.
    pub tags: Vec<TaskTag>,
    /// Assigned session.
    pub assigned_to: Option<String>,
    /// Whether selected.
    pub is_selected: bool,
    /// Whether hovered.
    pub is_hovered: bool,
    /// Available actions (shown on hover).
    pub actions: Vec<TaskItemAction>,
    /// Estimated time.
    pub estimated_time: Option<String>,
    /// Item height.
    pub height: f32,
}

impl TaskItem {
    /// Default item height.
    pub const DEFAULT_HEIGHT: f32 = 72.0;

    /// Generate rendering hints.
    pub fn render_hints(&self) -> TaskItemRenderHints {
        TaskItemRenderHints {
            id: self.id.clone(),
            title: self.title.clone(),
            priority: self.priority_indicator(),
            status: self.status_badge(),
            tags: self.tags.clone(),
            assigned_to: self.assigned_to.clone(),
            is_selected: self.is_selected,
            is_hovered: self.is_hovered,
            actions: self.available_actions(),
            estimated_time: self.estimated_time.clone(),
            height: Self::DEFAULT_HEIGHT,
        }
    }
}
