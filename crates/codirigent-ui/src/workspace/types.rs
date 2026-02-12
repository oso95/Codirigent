//! Type definitions for workspace UI components.
//!
//! This module contains struct and enum definitions used throughout the workspace
//! implementation, including modal states and UI component data.

use codirigent_core::{SessionId, TaskId};
use std::path::PathBuf;

/// Predefined group color palette for visual distinction.
///
/// These colors are used to assign distinct colors to session groups,
/// cycling through the palette as new groups are created.
pub(super) const GROUP_COLOR_PALETTE: &[&str] = &[
    "#f43f5e", // Rose
    "#8b5cf6", // Violet
    "#06b6d4", // Cyan
    "#f59e0b", // Amber
    "#10b981", // Emerald
    "#ec4899", // Pink
    "#3b82f6", // Blue
    "#84cc16", // Lime
    "#ef4444", // Red
    "#14b8a6", // Teal
];

/// Kind of session action being performed.
///
/// Used by the session action modal to determine which operation to perform
/// when the user submits the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionActionKind {
    /// Rename a session.
    Rename,
    /// Assign a session to a group.
    AssignGroup,
}

/// Session action modal state.
///
/// This modal appears when the user wants to rename a session or assign it to a group.
/// It captures a single text input and applies it based on the action kind.
#[derive(Debug, Clone)]
pub(super) struct SessionActionModal {
    /// Session being acted upon.
    pub(super) session_id: SessionId,
    /// Type of action (rename or assign to group).
    pub(super) kind: SessionActionKind,
    /// User input value.
    pub(super) input: String,
    /// Optional error message if validation fails.
    pub(super) error: Option<String>,
}

/// Task creation/edit modal state.
///
/// This modal is used both for creating new tasks and editing existing ones.
/// When `editing_task_id` is Some, it's an edit operation; when None, it's creating a new task.
#[derive(Debug, Clone)]
pub(super) struct TaskCreationModal {
    /// Task title.
    pub(super) title: String,
    /// Task description.
    pub(super) description: String,
    /// Task priority level.
    pub(super) priority: codirigent_core::TaskPriority,
    /// Currently focused form field (0=title, 1=description, 2=plan_file).
    pub(super) focused_field: usize,
    /// Optional error message if validation fails.
    pub(super) error: Option<String>,
    /// Project directory for this task.
    pub(super) project_dir: Option<PathBuf>,
    /// Plan file path (relative to project root).
    pub(super) plan_file: String,
    /// When editing an existing task, holds the task ID. None for new tasks.
    pub(super) editing_task_id: Option<TaskId>,
}

/// Context menu state for file tree right-click.
///
/// Captures the position and target of a file tree context menu invocation.
#[derive(Debug, Clone)]
pub(super) struct FileTreeContextMenu {
    /// Path of the right-clicked file/directory.
    pub(super) path: PathBuf,
    /// Screen position where the menu should appear.
    pub(super) position: gpui::Point<gpui::Pixels>,
}
