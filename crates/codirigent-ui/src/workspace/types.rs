//! Type definitions for workspace UI components.
//!
//! This module contains struct and enum definitions used throughout the workspace
//! implementation, including modal states and UI component data.

use codirigent_core::{SessionId, SessionStatus, TaskId};
use codirigent_session::claude_session_reader::ClaudeSessionReader;
use codirigent_session::codex_session_reader::CodexSessionReader;
use codirigent_session::gemini_session_reader::GeminiSessionReader;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

// --- Layout constants ---
// Shared across workspace rendering modules to avoid duplicate magic numbers.

/// Height of the session cell header bar in pixels.
pub(super) const HEADER_HEIGHT: f32 = 32.0;

/// Padding around the terminal canvas content in pixels.
/// Must match the padding used in render_terminal_content's canvas prepaint.
pub(super) const TERMINAL_CONTENT_PADDING: f32 = 4.0;

/// Total border width consumed by `.border_1()` on session cells (1px each side).
pub(super) const CELL_BORDER_WIDTH: f32 = 2.0;

/// Height of dropdown trigger buttons in pixels.
pub(super) const DROPDOWN_TRIGGER_HEIGHT: f32 = 28.0;

/// Default rem size in pixels (base for font scaling).
/// Scaled proportionally: `REM_BASE * (font_size_base / 13.0)`.
pub(super) const REM_BASE: f32 = 16.0;

/// Default font size base in pixels (used to compute rem scaling).
pub(super) const FONT_SIZE_BASE_DEFAULT: f32 = 13.0;

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

/// Grouped modal-related state for WorkspaceView.
///
/// Contains all fields related to modal dialogs (session action, task creation,
/// and profile deletion confirmation).
pub(super) struct ModalState {
    /// Session action modal state (rename/group).
    pub session_action: Option<SessionActionModal>,
    /// Task creation modal state.
    pub task_creation: Option<TaskCreationModal>,
    /// Pending layout profile deletion: (tab_index, profile_name) awaiting confirmation.
    pub pending_profile_deletion: Option<(usize, String)>,
}

impl ModalState {
    pub fn new() -> Self {
        Self {
            session_action: None,
            task_creation: None,
            pending_profile_deletion: None,
        }
    }
}

/// Grouped selection/interaction state for WorkspaceView.
///
/// Contains fields tracking user selection, context menus, and click state.
pub(super) struct SelectionState {
    /// Currently selected session ID (for context-follows-selection).
    pub selected_session_id: Option<SessionId>,
    /// Session menu state: which session's menu is open (if any).
    pub session_menu_open: Option<SessionId>,
    /// Whether the user is actively dragging a text selection in a terminal.
    pub is_selecting: bool,
    /// Session ID that is currently being selected in (for mouse move events).
    pub selecting_session_id: Option<SessionId>,
    /// File tree context menu state (path + screen position).
    pub file_tree_context_menu: Option<FileTreeContextMenu>,
    /// Click deduplication: track last click position and time to prevent double-creation.
    pub last_click_position: Option<(codirigent_core::GridPosition, Instant)>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            selected_session_id: None,
            session_menu_open: None,
            is_selecting: false,
            selecting_session_id: None,
            file_tree_context_menu: None,
            last_click_position: None,
        }
    }
}

/// Grouped adaptive polling state for WorkspaceView.
///
/// Contains fields tracking output polling frequency, resize throttling,
/// git refresh timing, and deferred enter keypresses.
pub(super) struct PollingState {
    /// Whether the last poll received output (for adaptive polling).
    pub last_poll_had_output: bool,
    /// Count of consecutive polls with no output (for adaptive polling).
    pub idle_poll_count: u32,
    /// Last time terminals were resized to grid (for throttling during drag).
    pub last_resize_time: Instant,
    /// Whether a deferred resize is pending.
    pub pending_resize: bool,
    /// Last time git status was refreshed for sessions.
    pub last_git_refresh: Instant,
    /// Last time JSONL status was checked (throttled to ~1/second).
    pub last_jsonl_check: Instant,
    /// Sessions that need a deferred Enter keypress sent to their PTY.
    pub pending_enters: HashMap<SessionId, (Instant, bool)>,
}

impl PollingState {
    pub fn new() -> Self {
        Self {
            last_poll_had_output: false,
            idle_poll_count: 0,
            last_resize_time: Instant::now() - std::time::Duration::from_millis(200),
            pending_resize: false,
            last_git_refresh: Instant::now(),
            last_jsonl_check: Instant::now(),
            pending_enters: HashMap::new(),
        }
    }
}

/// Grouped CLI session reader state for WorkspaceView.
///
/// Contains JSONL session readers for different CLI types and the
/// process-tree CLI detector.
pub(super) struct CliReaders {
    /// Claude Code JSONL session reader for high-fidelity status detection.
    pub claude: Option<ClaudeSessionReader>,
    /// Codex CLI JSONL session reader for high-fidelity status detection.
    pub codex: Option<CodexSessionReader>,
    /// Gemini CLI JSON session reader for high-fidelity status detection.
    pub gemini: Option<GeminiSessionReader>,
    /// Process-tree CLI detector for gating JSONL auto-probe on GenericShell sessions.
    pub detector: codirigent_session::DefaultCliDetector,
    /// Cached JSONL-derived session status, persisted between poll cycles so
    /// the high-frequency InputDetector does not overwrite it.
    pub cached_status: HashMap<SessionId, (SessionStatus, Option<String>)>,
}

impl CliReaders {
    pub fn new() -> Self {
        Self {
            claude: ClaudeSessionReader::new(),
            codex: CodexSessionReader::new(),
            gemini: GeminiSessionReader::new(),
            detector: codirigent_session::DefaultCliDetector::new(),
            cached_status: HashMap::new(),
        }
    }
}

/// Grouped cache state for WorkspaceView.
///
/// Contains cached detection results, PTY sizes, and other memoized state.
pub(super) struct CacheState {
    /// Cached monospace fonts detected from the system (populated lazily).
    pub monospace_fonts: Option<Vec<String>>,
    /// Last PTY-resized dimensions per session, used to skip redundant resize calls.
    pub pty_sizes: HashMap<SessionId, (u16, u16)>,
    /// Sessions that have received at least one manual task assignment.
    pub manually_assigned_sessions: HashSet<SessionId>,
    /// Tracks when compaction started per session (for timeout).
    pub compaction_start_times: HashMap<SessionId, Instant>,
    /// Tracks which session groups are expanded in the drawer's Sessions panel.
    pub drawer_group_expanded: HashMap<String, bool>,
}

impl CacheState {
    pub fn new() -> Self {
        Self {
            monospace_fonts: None,
            pty_sizes: HashMap::new(),
            manually_assigned_sessions: HashSet::new(),
            compaction_start_times: HashMap::new(),
            drawer_group_expanded: HashMap::new(),
        }
    }
}
