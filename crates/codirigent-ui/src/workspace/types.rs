//! Type definitions for workspace UI components.
//!
//! This module contains struct and enum definitions used throughout the workspace
//! implementation, including modal states and UI component data.

use crate::workspace::core::PaneDropTargetInfo;
use codirigent_core::{PaneId, SessionId, SessionStatus, SlotId, TaskId};
use codirigent_session::codex_session_reader::CodexSessionReader;
use codirigent_session::gemini_session_reader::GeminiSessionReader;
use gpui::Hsla;
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

/// Height of the drawer (Sessions panel) header bar in pixels.
pub(super) const DRAWER_HEADER_HEIGHT: f32 = 40.0;

/// Default prefix used when generating session names (e.g. "Session 1").
///
/// Used both for name generation (`format!("{}{}", SESSION_NAME_PREFIX, n)`)
/// and for reverse-parsing the session number (`strip_prefix(SESSION_NAME_PREFIX)`).
pub(super) const SESSION_NAME_PREFIX: &str = "Session ";

/// Label shown when a session uses the default shell resolution path.
pub(super) const SESSION_SHELL_AUTO_LABEL: &str = "Auto";

/// Label shown in shell pickers when the app should resolve the default shell automatically.
pub(super) const SHELL_PICKER_AUTO_DETECT_LABEL: &str = "(Auto-detect)";

/// Height of session and group rows in the Sessions drawer panel.
pub(super) const SESSION_ROW_HEIGHT: f32 = 28.0;

/// Height of rows in the Sessions drawer list.
pub(super) const SESSION_DRAWER_ROW_HEIGHT: f32 = 56.0;

/// Height of input fields and modal rows (larger than session rows).
pub(super) const MODAL_FIELD_HEIGHT: f32 = 36.0;

/// Git change status colors used in the Worktrees panel.
///
/// These are git-convention colors (green=staged, orange=modified, red=deleted,
/// blue=renamed/moved) and intentionally live outside the general app theme.
pub(super) mod git_colors {
    use gpui::Hsla;
    pub const STAGED: Hsla = Hsla {
        h: 0.35,
        s: 0.6,
        l: 0.5,
        a: 1.0,
    };
    pub const MODIFIED: Hsla = Hsla {
        h: 0.1,
        s: 0.8,
        l: 0.6,
        a: 1.0,
    };
    pub const DELETED: Hsla = Hsla {
        h: 0.0,
        s: 0.7,
        l: 0.55,
        a: 1.0,
    };
    pub const RENAMED: Hsla = Hsla {
        h: 0.58,
        s: 0.5,
        l: 0.6,
        a: 1.0,
    };
}

/// Default rem size in pixels (base for font scaling).
/// Scaled proportionally: `REM_BASE * (font_size_base / 13.0)`.
pub(super) const REM_BASE: f32 = 16.0;

/// Default font size base in pixels (used to compute rem scaling).
pub(super) const FONT_SIZE_BASE_DEFAULT: f32 = 13.0;

/// Label shown in empty grid cells (no session assigned).
pub(super) const EMPTY_CELL_MESSAGE: &str = "Idle - Ready for next task";

/// Semi-transparent black overlay used behind modal dialogs.
pub(super) const MODAL_BACKDROP: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.0,
    a: 0.5,
};

/// Background color for destructive action buttons (delete, end session).
pub(super) const DESTRUCTIVE_BUTTON_BG: Hsla = Hsla {
    h: 0.0,
    s: 0.8,
    l: 0.5,
    a: 1.0,
};

/// Hover background color for destructive action buttons (slightly darker).
pub(super) const DESTRUCTIVE_BUTTON_HOVER: Hsla = Hsla {
    h: 0.0,
    s: 0.8,
    l: 0.4,
    a: 1.0,
};

/// Hover background color for secondary/cancel buttons (subtle white tint).
pub(super) const CANCEL_BUTTON_HOVER: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 1.0,
    a: 0.1,
};

/// Muted grey used for git branch name labels in the worktrees panel.
pub(super) const BRANCH_NAME_COLOR: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.75,
    a: 1.0,
};

/// Light red used for destructive hover text (close-tab button, etc.).
///
/// Lighter than `DESTRUCTIVE_BUTTON_BG` to work as foreground text color.
pub(super) const DESTRUCTIVE_HOVER_TEXT: Hsla = Hsla {
    h: 0.0,
    s: 0.8,
    l: 0.6,
    a: 1.0,
};

/// Destructive action color for menu item text (softer red; used for "End Session" etc.).
///
/// Distinct from `DESTRUCTIVE_BUTTON_BG` which is used as a button fill.
/// The lighter lightness makes it readable as colored text against panel backgrounds.
pub(super) const DESTRUCTIVE_ITEM_COLOR: Hsla = Hsla {
    h: 0.0,
    s: 0.7,
    l: 0.55,
    a: 1.0,
};

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
    /// Cursor position (char index) within `input`.
    pub(super) cursor_position: usize,
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
    /// Cursor position (char index) for each field:
    /// [title, description, plan_file].
    pub(super) cursor_positions: [usize; 3],
    /// Optional error message if validation fails.
    pub(super) error: Option<String>,
    /// Project directory for this task.
    pub(super) project_dir: Option<PathBuf>,
    /// Plan file path (relative to project root).
    pub(super) plan_file: String,
    /// When editing an existing task, holds the task ID. None for new tasks.
    pub(super) editing_task_id: Option<TaskId>,
}

/// Session creation modal state.
///
/// This modal is used for every session creation entry point so shell selection
/// stays consistent whether the user clicks an empty pane, a pane-local `+`,
/// or the generic create action.
#[derive(Debug, Clone)]
pub(super) struct SessionCreationModal {
    /// Pane that should receive the created session, if any.
    pub(super) target_pane: Option<PaneId>,
    /// Stored shell values; empty string means Auto.
    pub(super) shell_options: Vec<String>,
    /// Currently selected option index.
    pub(super) selected_shell_index: usize,
    /// Whether a background create request is currently in flight.
    pub(super) pending: bool,
    /// Optional validation or creation error.
    pub(super) error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ShellPickerOption {
    pub(super) source_index: usize,
    pub(super) raw_value: String,
    pub(super) label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ShellPickerSection {
    pub(super) title: Option<&'static str>,
    pub(super) options: Vec<ShellPickerOption>,
}

#[derive(Debug, Clone)]
pub(super) struct RestoreShellFallback {
    /// Shell originally requested by the saved session.
    pub(super) requested_shell: String,
    /// User-facing label for the shell actually launched for the restored session.
    pub(super) effective_shell_label: String,
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
    /// Session creation modal state.
    pub session_creation: Option<SessionCreationModal>,
    /// Task creation modal state.
    pub task_creation: Option<TaskCreationModal>,
    /// Pending layout profile deletion: (tab_index, profile_name) awaiting confirmation.
    pub pending_profile_deletion: Option<(usize, String)>,
    /// Whether text cursors in modals should currently be visible.
    pub cursor_blink_on: bool,
}

impl ModalState {
    pub fn new() -> Self {
        Self {
            session_action: None,
            session_creation: None,
            task_creation: None,
            pending_profile_deletion: None,
            cursor_blink_on: true,
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
    /// Vertical anchor position for the session menu overlay, in window pixels.
    pub session_menu_anchor_y: Option<f32>,
    /// Horizontal anchor position for the session menu overlay, in window pixels.
    /// When `Some`, the menu is positioned at this X coordinate (e.g. tab right-click).
    /// When `None`, the menu uses the default drawer-relative positioning.
    pub session_menu_anchor_x: Option<f32>,
    /// Whether the user is actively dragging a text selection in a terminal.
    pub is_selecting: bool,
    /// Session ID that is currently being selected in (for mouse move events).
    pub selecting_session_id: Option<SessionId>,
    /// File tree context menu state (path + screen position).
    pub file_tree_context_menu: Option<FileTreeContextMenu>,
    /// Click deduplication: track last click position and time to prevent double-creation.
    pub last_click_position: Option<(codirigent_core::GridPosition, Instant)>,
    /// Active drag-and-drop state for session reordering (None when not dragging).
    pub drag: Option<DragState>,
    /// Active split-divider resize gesture (None when not resizing).
    pub split_resize: Option<SplitResizeState>,
    /// Active terminal scrollbar drag gesture (None when not dragging).
    pub terminal_scrollbar_drag: Option<TerminalScrollbarDragState>,
}

/// Active terminal scrollbar drag state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TerminalScrollbarDragState {
    /// Session owning the active scrollbar drag.
    pub session_id: SessionId,
    /// Screen-space Y coordinate where the track starts.
    pub track_top: f32,
    /// Height of the scrollbar track.
    pub track_height: f32,
}

/// State for drag-and-drop session reordering.
///
/// Tracks an in-progress drag operation where the user is moving a session
/// from one pane to another by dragging its header bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DragTargetKind {
    PaneBody,
    PaneHeader,
}

/// Current drop target under the pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DragTarget {
    /// Visible pane identifier under the pointer.
    pub pane_id: PaneId,
    /// Grid or split logical cell index.
    pub index: usize,
    /// Whether the pane currently has an active session.
    pub active_session_id: Option<SessionId>,
    /// Whether the pointer is over the header or body region.
    pub kind: DragTargetKind,
}

#[derive(Debug, Clone)]
pub(super) struct DragState {
    /// Session being dragged.
    pub source_session_id: SessionId,
    /// Visible pane the session originated from.
    pub source_pane_id: PaneId,
    /// Grid index (or slot index) of the source cell.
    pub source_index: usize,
    /// Mouse position when drag started (screen pixels).
    pub start_position: crate::layout::Point,
    /// Current mouse position (screen pixels).
    pub current_position: crate::layout::Point,
    /// Whether the drag threshold (5px) has been exceeded.
    pub active: bool,
    /// Cell currently under the cursor (drop target), if any.
    pub target: Option<DragTarget>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SplitResizeState {
    /// Representative slot from the first subtree of the resized split.
    pub first_slot: SlotId,
    /// Representative slot from the second subtree of the resized split.
    pub second_slot: SlotId,
    /// Direction of the split being resized.
    pub direction: codirigent_core::SplitDirection,
    /// Global bounds of the split container.
    pub bounds: crate::layout::Bounds,
    /// Gap thickness used for the divider.
    pub gap: f32,
    /// Pointer offset within the divider handle at drag start.
    pub grab_offset: f32,
    /// Whether the drag produced at least one ratio change.
    pub changed: bool,
}

const DRAG_ACTIVATION_DISTANCE_SQUARED: f32 = 25.0;

impl DragState {
    /// Update drag activation and drop target from a pointer position.
    ///
    /// This is shared between header-local and workspace-global mouse move
    /// handlers so reordering keeps working after the cursor leaves the
    /// source header.
    pub(super) fn update_pointer(
        &mut self,
        position: crate::layout::Point,
        panes: &[PaneDropTargetInfo],
    ) {
        self.current_position = position;

        if !self.active {
            let dx = position.x - self.start_position.x;
            let dy = position.y - self.start_position.y;
            if (dx * dx + dy * dy) <= DRAG_ACTIVATION_DISTANCE_SQUARED {
                self.target = None;
                return;
            }
            self.active = true;
        }

        self.target = panes
            .iter()
            .find(|pane| pane.bounds.contains(position))
            .and_then(|pane| {
                (pane.pane_id != self.source_pane_id).then(|| {
                    let header_bottom = pane.bounds.origin.y + HEADER_HEIGHT;
                    let kind = if position.y <= header_bottom {
                        DragTargetKind::PaneHeader
                    } else {
                        DragTargetKind::PaneBody
                    };
                    DragTarget {
                        pane_id: pane.pane_id.clone(),
                        index: pane.index,
                        active_session_id: pane.active_session_id,
                        kind,
                    }
                })
            });
    }
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            selected_session_id: None,
            session_menu_open: None,
            session_menu_anchor_y: None,
            session_menu_anchor_x: None,
            is_selecting: false,
            selecting_session_id: None,
            file_tree_context_menu: None,
            last_click_position: None,
            drag: None,
            split_resize: None,
            terminal_scrollbar_drag: None,
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
    /// Sessions that need a deferred Enter keypress sent to their PTY.
    pub pending_enters: HashMap<SessionId, (Instant, bool)>,
    /// Last time clipboard was checked for changes (time-based, ~1/second).
    pub last_clipboard_check: Instant,
    /// Whether a background git refresh is currently in-flight.
    pub git_refresh_in_flight: bool,
    /// Whether a background JSONL status check (Codex/Gemini) is in-flight.
    pub jsonl_check_in_flight: bool,
    /// Whether a background hook-signal scan is in-flight.
    pub hook_signal_check_in_flight: bool,
    /// Whether a background file tree rebuild is currently in-flight.
    pub file_tree_rebuild_in_flight: bool,
    /// Whether a background clipboard image save is currently in-flight.
    pub clipboard_load_in_flight: bool,
    /// Whether a background detector maintenance job is currently in-flight.
    pub detector_maintenance_in_flight: bool,
    /// Sessions currently preparing PTY output on a background thread.
    pub output_prepare_in_flight: HashSet<SessionId>,
    /// Debounced app-state persistence task.
    pub state_save_task: Option<gpui::Task<()>>,
    /// Monotonic generation for debounced app-state persistence.
    pub state_save_generation: u64,
    /// Last time the Codex/Gemini JSONL check ran.
    pub last_jsonl_check: Instant,
    /// Last time hook signal files were scanned (~1/second throttle).
    pub last_hook_signal_check: Instant,
    /// Latest hook payload marker processed per signal file stem.
    pub last_processed_hook_signal_ts: HashMap<String, ProcessedHookSignal>,
    /// Generation counter for async project-root refreshes (file tree/worktree).
    pub project_refresh_generation: u64,
    /// Whether session restoration from disk is currently in-flight.
    pub restore_in_flight: bool,
    /// Reserved session numbers for session-create bootstraps that have not
    /// completed yet. Prevents duplicate names when users create sessions
    /// faster than the background PTY bootstrap finishes.
    pub pending_session_bootstrap_numbers: HashSet<u64>,
    /// Reserved split-tree slots for session-create bootstraps that have not
    /// completed yet. Prevents duplicate creates racing into the same slot.
    pub pending_session_bootstrap_slots: HashSet<SlotId>,
    /// Last time the legacy fallback safety net drained pending_output_sessions.
    pub last_legacy_fallback: Instant,
    /// Best-effort shell command line capture per session while the shell is idle.
    pub shell_input_buffers: HashMap<SessionId, String>,
    /// CLI resume commands waiting for the shell to produce output before
    /// being dispatched. Keyed by session ID; value is (enqueued_at, commands).
    /// Commands are sent as soon as the first PTY output is received (the shell
    /// is alive) or after `RESUME_COMMAND_FALLBACK_TIMEOUT` as a safety net.
    pub pending_resume_commands: HashMap<SessionId, (Instant, Vec<String>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProcessedHookSignal {
    pub ts: u64,
    pub fingerprint: u64,
}

impl PollingState {
    pub fn new() -> Self {
        Self {
            last_poll_had_output: false,
            idle_poll_count: 0,
            last_resize_time: Instant::now() - std::time::Duration::from_millis(200),
            pending_resize: false,
            last_git_refresh: Instant::now(),
            pending_enters: HashMap::new(),
            last_clipboard_check: Instant::now(),
            git_refresh_in_flight: false,
            jsonl_check_in_flight: false,
            hook_signal_check_in_flight: false,
            file_tree_rebuild_in_flight: false,
            clipboard_load_in_flight: false,
            detector_maintenance_in_flight: false,
            output_prepare_in_flight: HashSet::new(),
            state_save_task: None,
            state_save_generation: 0,
            last_jsonl_check: Instant::now(),
            last_hook_signal_check: Instant::now() - std::time::Duration::from_secs(1),
            last_processed_hook_signal_ts: HashMap::new(),
            project_refresh_generation: 0,
            restore_in_flight: false,
            pending_session_bootstrap_numbers: HashSet::new(),
            pending_session_bootstrap_slots: HashSet::new(),
            last_legacy_fallback: Instant::now(),
            shell_input_buffers: HashMap::new(),
            pending_resume_commands: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CliStatusSource {
    Hook,
    Jsonl,
}

#[derive(Debug, Clone)]
pub(super) struct CachedCliStatus {
    pub(super) status: SessionStatus,
    pub(super) seen_at: Instant,
    pub(super) source: CliStatusSource,
    /// When the status last changed (for stale NeedsAttention detection).
    pub(super) status_since: Instant,
    /// How long this entry is valid after `seen_at`.
    /// Hook-based entries (Claude Code) use a longer TTL than JSONL-based ones
    /// so a long-running task does not lose its "working" state between polls.
    pub(super) ttl: std::time::Duration,
}

/// Grouped CLI session reader state for WorkspaceView.
///
/// Contains JSONL/session readers for Codex and Gemini plus the process-tree
/// CLI detector. Claude Code and Gemini can receive hook-based status updates
/// via hook signal processing; the readers remain as a higher-fidelity fallback.
pub(super) struct CliReaders {
    /// Codex CLI JSONL session reader for high-fidelity status detection.
    pub codex: Option<CodexSessionReader>,
    /// Gemini CLI JSON session reader for high-fidelity status detection.
    pub gemini: Option<GeminiSessionReader>,
    /// Process-tree CLI detector for gating JSONL auto-probe on GenericShell sessions.
    pub detector: codirigent_session::DefaultCliDetector,
    /// Cached CLI-derived session status, persisted between poll cycles so
    /// the high-frequency InputDetector does not overwrite it.
    pub cached_status: HashMap<SessionId, CachedCliStatus>,
}

impl CliReaders {
    pub fn new() -> Self {
        Self {
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
    /// Cached installed editors detected from the system (populated in background on init).
    pub detected_editors: Option<Vec<String>>,
    /// Cached available shells detected from the system (populated in background on init).
    pub detected_shells: Option<Vec<String>>,
    /// Effective shell labels for running sessions, used so "Auto" resolves to the real shell.
    pub effective_shell_labels: HashMap<SessionId, String>,
    /// Sessions restored with a fallback shell because their requested shell was unavailable.
    pub restore_shell_fallbacks: HashMap<SessionId, RestoreShellFallback>,
    /// Last PTY-resized dimensions per session, used to skip redundant resize calls.
    pub pty_sizes: HashMap<SessionId, (u16, u16)>,
    /// Sessions that have received at least one manual task assignment.
    pub manually_assigned_sessions: HashSet<SessionId>,
    /// Tracks when compaction started per session (for timeout).
    pub compaction_start_times: HashMap<SessionId, Instant>,
    /// Tracks which session groups are expanded in the drawer's Sessions panel.
    pub drawer_group_expanded: HashMap<String, bool>,
    /// Cached result of font metric computation, keyed by font settings.
    /// Avoids repeated font system calls when settings haven't changed.
    pub cached_cell_dims: Option<CachedCellDims>,
    /// Cached cell layout info reused by resize and paint passes.
    pub render_cell_info: Vec<super::core::CellInfo>,
    /// Cached pane bounds reused by drag/drop hit testing.
    pub render_pane_drop_targets: Vec<PaneDropTargetInfo>,
    /// Whether `render_cell_info` must be recomputed before use.
    pub render_cell_info_dirty: bool,
    /// Last geometry signature used to build `render_cell_info`.
    pub render_layout_signature: Option<RenderLayoutSignature>,
    /// Monotonic generation for layout/session arrangement changes.
    pub layout_generation: u64,
    /// Last signature applied to terminal resize sync.
    pub last_resize_signature: Option<TerminalResizeSignature>,
    /// Latest pending resize signature queued for the deferred resize path.
    pub pending_resize_signature: Option<TerminalResizeSignature>,
    /// Last captured window bounds for persistence and change detection.
    pub last_window_state: Option<codirigent_core::WindowState>,
}

impl CacheState {
    pub fn new() -> Self {
        Self {
            monospace_fonts: None,
            detected_editors: None,
            detected_shells: None,
            effective_shell_labels: HashMap::new(),
            restore_shell_fallbacks: HashMap::new(),
            pty_sizes: HashMap::new(),
            manually_assigned_sessions: HashSet::new(),
            compaction_start_times: HashMap::new(),
            drawer_group_expanded: HashMap::new(),
            cached_cell_dims: None,
            render_cell_info: Vec::new(),
            render_pane_drop_targets: Vec::new(),
            render_cell_info_dirty: true,
            render_layout_signature: None,
            layout_generation: 0,
            last_resize_signature: None,
            pending_resize_signature: None,
            last_window_state: None,
        }
    }
}

/// Cached result of font metric computation, keyed by font settings.
#[derive(Debug, Clone)]
pub(super) struct CachedCellDims {
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub centering_adjust: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RenderLayoutSignature {
    pub bounds: crate::layout::Bounds,
    pub sidebar_width: f32,
    pub right_panel_width: f32,
    pub grid_gap: f32,
    /// Hash of the sessions actively rendered in visible panes, in pane order.
    pub rendered_sessions_signature: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TerminalResizeSignature {
    pub layout_generation: u64,
    pub layout: RenderLayoutSignature,
    pub cell_width: f32,
    pub cell_height: f32,
}
