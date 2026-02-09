//! Event types for cross-module communication.
//!
//! This module defines the [`CodirigentEvent`] enum which is used for
//! loose coupling between modules. All cross-module communication
//! should happen through events, allowing modules to react to changes
//! without tight coupling.

use crate::session_advanced::{OvernightConfig, OvernightSummary};
use crate::skill::TokenBudget;
use crate::types::*;
use std::path::PathBuf;

/// Type of content detected on clipboard.
///
/// Used to determine how to handle clipboard content when pasting
/// into sessions or saving to files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardContentType {
    /// Plain text content.
    Text,
    /// Image data (PNG/JPEG).
    Image,
    /// File paths.
    Files,
    /// Empty clipboard.
    Empty,
}

/// Events for loose coupling between modules.
///
/// All cross-module communication should happen through events,
/// allowing modules to react to changes without tight coupling.
///
/// # Event Categories
///
/// - **Session Events**: Session lifecycle and state changes
/// - **Input Detection Events**: User input requirements
/// - **Layout Events**: UI layout changes
/// - **Task Events**: Task lifecycle (Phase 2 foundation)
/// - **File Tree Events**: File drag-and-drop operations
/// - **Skill Events**: Skill management and token budget tracking
///
/// # Example
///
/// ```
/// use codirigent_core::events::CodirigentEvent;
/// use codirigent_core::types::{SessionId, SessionStatus};
///
/// let event = CodirigentEvent::SessionStatusChanged {
///     id: SessionId(1),
///     old: SessionStatus::Idle,
///     new: SessionStatus::Working,
/// };
/// ```
#[derive(Debug, Clone)]
pub enum CodirigentEvent {
    // === Session Events ===
    /// A new session was created.
    SessionCreated {
        /// The ID of the newly created session.
        id: SessionId,
    },

    /// A session was closed.
    SessionClosed {
        /// The ID of the closed session.
        id: SessionId,
    },

    /// Session status changed (detected by Input Detector).
    SessionStatusChanged {
        /// The session ID.
        id: SessionId,
        /// The previous status.
        old: SessionStatus,
        /// The new status.
        new: SessionStatus,
    },

    /// Output was received from a session's PTY.
    SessionOutputReceived {
        /// The session ID.
        id: SessionId,
        /// The raw output data.
        data: Vec<u8>,
    },

    /// Session was renamed.
    SessionRenamed {
        /// The session ID.
        id: SessionId,
        /// The old name.
        old_name: String,
        /// The new name.
        new_name: String,
    },

    /// Session group/color changed.
    SessionGroupChanged {
        /// The session ID.
        id: SessionId,
        /// The new group name (None to ungroup).
        group: Option<String>,
        /// The new color (None to use default).
        color: Option<String>,
    },

    // === Input Detection Events ===
    /// Input is required from the user (detected pattern).
    InputRequired {
        /// The session ID.
        session_id: SessionId,
        /// The pattern that triggered the detection.
        pattern: Option<String>,
    },

    /// User provided input (pattern resolved).
    InputProvided {
        /// The session ID.
        session_id: SessionId,
    },

    /// A Claude Code session needs tool permission approval.
    PermissionRequired {
        /// The session ID.
        session_id: SessionId,
        /// The tool name that needs permission, if known.
        tool_name: Option<String>,
    },

    // === Layout Events ===
    /// Layout mode changed.
    LayoutChanged {
        /// The new layout mode.
        mode: LayoutMode,
    },

    /// A session was focused.
    SessionFocused {
        /// The focused session ID.
        id: SessionId,
    },

    // === Task Events (Phase 2 foundation) ===
    /// A new task was created.
    TaskCreated {
        /// The task ID.
        id: TaskId,
    },

    /// A task was assigned to a session.
    TaskAssigned {
        /// The task ID.
        task_id: TaskId,
        /// The session ID.
        session_id: SessionId,
    },

    /// A task was completed.
    TaskCompleted {
        /// The task ID.
        task_id: TaskId,
        /// Whether the task succeeded.
        success: bool,
    },

    // === File Tree Events ===
    /// Path was dragged to a session.
    PathDraggedToSession {
        /// The session ID.
        session_id: SessionId,
        /// The path that was dragged.
        path: PathBuf,
    },

    // === Worktree Events ===
    /// A new worktree was created.
    WorktreeCreated {
        /// Absolute path to the worktree directory.
        path: PathBuf,
        /// Branch name associated with this worktree.
        branch: String,
    },

    /// A worktree was removed.
    WorktreeRemoved {
        /// Absolute path to the removed worktree.
        path: PathBuf,
    },

    /// A session was bound to a worktree.
    SessionBoundToWorktree {
        /// The session ID.
        session_id: SessionId,
        /// Absolute path to the worktree.
        worktree_path: PathBuf,
    },

    /// A session was unbound from its worktree.
    SessionUnboundFromWorktree {
        /// The session ID.
        session_id: SessionId,
    },

    // === Context Tracking Events ===
    /// Context usage was updated for a session.
    ContextUsageUpdated {
        /// The session ID.
        session_id: SessionId,
        /// Raw context usage percentage (0.0-1.0).
        percentage: f32,
        /// Effective context usage after MCP overhead (0.0-1.0).
        effective_percentage: f32,
    },

    /// Context threshold was reached (warning or critical).
    ContextThresholdReached {
        /// The session ID.
        session_id: SessionId,
        /// The threshold value that was reached (0.0-1.0).
        threshold: f32,
        /// The threshold state (Warning or Critical).
        state: ContextThresholdState,
    },

    // === Skill Events ===
    /// A skill was enabled.
    SkillEnabled {
        /// Name of the enabled skill.
        name: String,
    },

    /// A skill was disabled.
    SkillDisabled {
        /// Name of the disabled skill.
        name: String,
    },

    /// Token budget warning threshold reached.
    TokenBudgetWarning {
        /// Current budget state.
        budget: TokenBudget,
    },

    /// Token budget exceeded.
    TokenBudgetExceeded {
        /// Current budget state.
        budget: TokenBudget,
    },

    /// A skill preset was applied.
    SkillPresetApplied {
        /// Name of the applied preset.
        preset_name: String,
        /// Number of skills enabled.
        enabled_count: usize,
    },

    /// Skills were refreshed from disk.
    SkillsRefreshed {
        /// Total number of skills found.
        count: usize,
    },

    // === Ralph Loop Events ===
    /// A Ralph Loop was started for a session.
    RalphLoopStarted {
        /// The session ID.
        session_id: SessionId,
        /// The configuration for the loop.
        config: crate::ralph::RalphLoopConfig,
    },

    /// A Ralph Loop was paused.
    RalphLoopPaused {
        /// The session ID.
        session_id: SessionId,
    },

    /// A Ralph Loop was resumed.
    RalphLoopResumed {
        /// The session ID.
        session_id: SessionId,
    },

    /// A Ralph Loop was cancelled.
    RalphLoopCancelled {
        /// The session ID.
        session_id: SessionId,
    },

    /// A Ralph Loop completed an iteration.
    RalphLoopIteration {
        /// The session ID.
        session_id: SessionId,
        /// The iteration number.
        iteration: u32,
        /// Whether verification passed.
        passed: bool,
        /// Number of test failures (if applicable).
        test_failures: Option<u32>,
    },

    /// A Ralph Loop completed successfully.
    RalphLoopCompleted {
        /// The session ID.
        session_id: SessionId,
        /// Total number of iterations executed.
        total_iterations: u32,
    },

    /// A Ralph Loop failed.
    RalphLoopFailed {
        /// The session ID.
        session_id: SessionId,
        /// Reason for failure.
        reason: String,
        /// Number of iterations executed.
        iterations: u32,
    },

    /// A Ralph Loop detected a stuck state.
    RalphLoopStuck {
        /// The session ID.
        session_id: SessionId,
        /// Number of iterations without progress.
        iterations_without_progress: u32,
    },

    /// Context compaction was triggered for a Ralph Loop.
    RalphLoopCompacted {
        /// The session ID.
        session_id: SessionId,
        /// The iteration at which compaction was triggered.
        iteration: u32,
    },

    // === Broadcast Events ===
    /// Broadcast message was sent.
    BroadcastSent {
        /// Broadcast ID.
        id: crate::broadcast::BroadcastId,
        /// Number of target sessions.
        target_count: usize,
        /// Message priority.
        priority: crate::broadcast::BroadcastPriority,
    },

    /// Broadcast was delivered to a session.
    BroadcastDelivered {
        /// Broadcast ID.
        id: crate::broadcast::BroadcastId,
        /// Session that received the message.
        session_id: SessionId,
    },

    /// Broadcast delivery failed for a session.
    BroadcastDeliveryFailed {
        /// Broadcast ID.
        id: crate::broadcast::BroadcastId,
        /// Session that failed to receive.
        session_id: SessionId,
        /// Error message.
        error: String,
    },

    /// All broadcast deliveries completed.
    BroadcastComplete {
        /// Broadcast ID.
        id: crate::broadcast::BroadcastId,
        /// Number of successful deliveries.
        success_count: usize,
        /// Number of failed deliveries.
        failure_count: usize,
    },

    // === Advanced Session Events ===
    /// Context handoff was initiated between sessions.
    HandoffInitiated {
        /// Source session (high context usage).
        source: SessionId,
        /// Target session (new or low context).
        target: SessionId,
    },

    /// Context handoff completed successfully.
    HandoffCompleted {
        /// Source session.
        source: SessionId,
        /// Target session.
        target: SessionId,
    },

    /// Context handoff failed.
    HandoffFailed {
        /// Source session.
        source: SessionId,
        /// Target session.
        target: SessionId,
        /// Error message.
        error: String,
    },

    /// Session was created from a template.
    SessionCreatedFromTemplate {
        /// The created session ID.
        session_id: SessionId,
        /// Name of the template used.
        template_name: String,
    },

    /// Session template was saved.
    TemplateSaved {
        /// Name of the saved template.
        name: String,
    },

    /// Session was cloned.
    SessionCloned {
        /// Original session ID.
        source_session: SessionId,
        /// New session ID.
        cloned_session: SessionId,
    },

    /// Session group was created.
    SessionGroupCreated {
        /// Group name.
        name: String,
    },

    /// Session was added to a group.
    SessionAddedToGroup {
        /// Session ID.
        session_id: SessionId,
        /// Group name.
        group_name: String,
    },

    /// Session was removed from a group.
    SessionRemovedFromGroup {
        /// Session ID.
        session_id: SessionId,
        /// Group name.
        group_name: String,
    },

    // === Overnight Mode Events ===
    /// Overnight mode was started.
    OvernightStarted {
        /// Configuration used.
        config: OvernightConfig,
    },

    /// Overnight mode was stopped.
    OvernightStopped {
        /// Summary of work done.
        summary: OvernightSummary,
    },

    /// Task completed during overnight mode.
    OvernightTaskCompleted {
        /// Task number in the overnight session.
        task_number: u32,
        /// Whether the task succeeded.
        success: bool,
    },

    /// Error occurred during overnight mode.
    OvernightError {
        /// Error message.
        error: String,
        /// Task number if applicable.
        task_number: Option<u32>,
    },

    // === Working Directory Events ===
    /// Session working directory changed (detected via OSC 7).
    WorkingDirectoryChanged {
        /// The session ID.
        id: SessionId,
        /// The previous working directory.
        old_dir: PathBuf,
        /// The new working directory.
        new_dir: PathBuf,
    },

    // === Git Status Events ===
    /// Git status was updated for a session.
    GitStatusUpdated {
        /// The session ID.
        session_id: SessionId,
        /// Updated git info (None if not in a git repo).
        git_info: Option<crate::types::GitRepoInfo>,
    },

    // === Clipboard Events ===
    /// Clipboard content was detected.
    ClipboardContentDetected {
        /// Type of content detected.
        content_type: ClipboardContentType,
    },

    /// Image was saved from clipboard.
    ClipboardImageSaved {
        /// Session ID (if focused).
        session_id: Option<SessionId>,
        /// Path where image was saved.
        path: PathBuf,
    },

    /// Smart paste was triggered.
    SmartPasteTriggered {
        /// Target session ID.
        session_id: SessionId,
        /// Formatted content ready for pasting.
        formatted_content: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_created_event() {
        let event = CodirigentEvent::SessionCreated { id: SessionId(1) };
        assert!(matches!(event, CodirigentEvent::SessionCreated { .. }));
    }

    #[test]
    fn test_session_closed_event() {
        let event = CodirigentEvent::SessionClosed { id: SessionId(42) };
        if let CodirigentEvent::SessionClosed { id } = event {
            assert_eq!(id, SessionId(42));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_status_changed_event() {
        let event = CodirigentEvent::SessionStatusChanged {
            id: SessionId(1),
            old: SessionStatus::Idle,
            new: SessionStatus::Working,
        };
        if let CodirigentEvent::SessionStatusChanged { old, new, .. } = event {
            assert_eq!(old, SessionStatus::Idle);
            assert_eq!(new, SessionStatus::Working);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_output_received_event() {
        let data = vec![72, 101, 108, 108, 111]; // "Hello"
        let event = CodirigentEvent::SessionOutputReceived {
            id: SessionId(1),
            data: data.clone(),
        };
        if let CodirigentEvent::SessionOutputReceived {
            id,
            data: received_data,
        } = event
        {
            assert_eq!(id, SessionId(1));
            assert_eq!(received_data, data);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_renamed_event() {
        let event = CodirigentEvent::SessionRenamed {
            id: SessionId(1),
            old_name: "old".to_string(),
            new_name: "new".to_string(),
        };
        if let CodirigentEvent::SessionRenamed {
            id,
            old_name,
            new_name,
        } = event
        {
            assert_eq!(id, SessionId(1));
            assert_eq!(old_name, "old");
            assert_eq!(new_name, "new");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_group_changed_event() {
        let event = CodirigentEvent::SessionGroupChanged {
            id: SessionId(1),
            group: Some("backend".to_string()),
            color: Some("#FF0000".to_string()),
        };
        if let CodirigentEvent::SessionGroupChanged { id, group, color } = event {
            assert_eq!(id, SessionId(1));
            assert_eq!(group, Some("backend".to_string()));
            assert_eq!(color, Some("#FF0000".to_string()));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_input_required_event() {
        let event = CodirigentEvent::InputRequired {
            session_id: SessionId(1),
            pattern: Some("y/n".to_string()),
        };
        if let CodirigentEvent::InputRequired {
            session_id,
            pattern,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(pattern, Some("y/n".to_string()));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_input_required_event_no_pattern() {
        let event = CodirigentEvent::InputRequired {
            session_id: SessionId(1),
            pattern: None,
        };
        if let CodirigentEvent::InputRequired { pattern, .. } = event {
            assert!(pattern.is_none());
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_input_provided_event() {
        let event = CodirigentEvent::InputProvided {
            session_id: SessionId(1),
        };
        assert!(matches!(event, CodirigentEvent::InputProvided { .. }));
    }

    #[test]
    fn test_layout_changed_event() {
        let event = CodirigentEvent::LayoutChanged {
            mode: LayoutMode::Grid { rows: 2, cols: 3 },
        };
        if let CodirigentEvent::LayoutChanged { mode } = event {
            assert!(matches!(mode, LayoutMode::Grid { rows: 2, cols: 3 }));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_focused_event() {
        let event = CodirigentEvent::SessionFocused { id: SessionId(1) };
        assert!(matches!(event, CodirigentEvent::SessionFocused { .. }));
    }

    #[test]
    fn test_task_created_event() {
        let event = CodirigentEvent::TaskCreated {
            id: TaskId("task-001".to_string()),
        };
        if let CodirigentEvent::TaskCreated { id } = event {
            assert_eq!(id, TaskId("task-001".to_string()));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_task_assigned_event() {
        let event = CodirigentEvent::TaskAssigned {
            task_id: TaskId("task-001".to_string()),
            session_id: SessionId(1),
        };
        if let CodirigentEvent::TaskAssigned {
            task_id,
            session_id,
        } = event
        {
            assert_eq!(task_id, TaskId("task-001".to_string()));
            assert_eq!(session_id, SessionId(1));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_task_completed_event_success() {
        let event = CodirigentEvent::TaskCompleted {
            task_id: TaskId("task-001".to_string()),
            success: true,
        };
        if let CodirigentEvent::TaskCompleted { task_id, success } = event {
            assert_eq!(task_id, TaskId("task-001".to_string()));
            assert!(success);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_task_completed_event_failure() {
        let event = CodirigentEvent::TaskCompleted {
            task_id: TaskId("task-001".to_string()),
            success: false,
        };
        if let CodirigentEvent::TaskCompleted { success, .. } = event {
            assert!(!success);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_path_dragged_to_session_event() {
        let event = CodirigentEvent::PathDraggedToSession {
            session_id: SessionId(1),
            path: PathBuf::from("/home/user/file.txt"),
        };
        if let CodirigentEvent::PathDraggedToSession { session_id, path } = event {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(path, PathBuf::from("/home/user/file.txt"));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_worktree_created_event() {
        let event = CodirigentEvent::WorktreeCreated {
            path: PathBuf::from("/repo/worktrees/feature"),
            branch: "feature-branch".to_string(),
        };
        if let CodirigentEvent::WorktreeCreated { path, branch } = event {
            assert_eq!(path, PathBuf::from("/repo/worktrees/feature"));
            assert_eq!(branch, "feature-branch");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_worktree_removed_event() {
        let event = CodirigentEvent::WorktreeRemoved {
            path: PathBuf::from("/repo/worktrees/feature"),
        };
        if let CodirigentEvent::WorktreeRemoved { path } = event {
            assert_eq!(path, PathBuf::from("/repo/worktrees/feature"));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_bound_to_worktree_event() {
        let event = CodirigentEvent::SessionBoundToWorktree {
            session_id: SessionId(1),
            worktree_path: PathBuf::from("/repo/worktrees/feature"),
        };
        if let CodirigentEvent::SessionBoundToWorktree {
            session_id,
            worktree_path,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(worktree_path, PathBuf::from("/repo/worktrees/feature"));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_unbound_from_worktree_event() {
        let event = CodirigentEvent::SessionUnboundFromWorktree {
            session_id: SessionId(1),
        };
        if let CodirigentEvent::SessionUnboundFromWorktree { session_id } = event {
            assert_eq!(session_id, SessionId(1));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_event_clone() {
        let event = CodirigentEvent::SessionFocused { id: SessionId(1) };
        let cloned = event.clone();
        assert!(matches!(cloned, CodirigentEvent::SessionFocused { .. }));
    }

    #[test]
    fn test_event_debug() {
        let event = CodirigentEvent::SessionCreated { id: SessionId(42) };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("SessionCreated"));
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_context_usage_updated_event() {
        let event = CodirigentEvent::ContextUsageUpdated {
            session_id: SessionId(1),
            percentage: 0.5,
            effective_percentage: 0.625,
        };
        if let CodirigentEvent::ContextUsageUpdated {
            session_id,
            percentage,
            effective_percentage,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert!((percentage - 0.5).abs() < f32::EPSILON);
            assert!((effective_percentage - 0.625).abs() < f32::EPSILON);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_context_threshold_reached_event_warning() {
        let event = CodirigentEvent::ContextThresholdReached {
            session_id: SessionId(1),
            threshold: 0.7,
            state: ContextThresholdState::Warning,
        };
        if let CodirigentEvent::ContextThresholdReached {
            session_id,
            threshold,
            state,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert!((threshold - 0.7).abs() < f32::EPSILON);
            assert_eq!(state, ContextThresholdState::Warning);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_context_threshold_reached_event_critical() {
        let event = CodirigentEvent::ContextThresholdReached {
            session_id: SessionId(1),
            threshold: 0.9,
            state: ContextThresholdState::Critical,
        };
        if let CodirigentEvent::ContextThresholdReached {
            session_id,
            threshold,
            state,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert!((threshold - 0.9).abs() < f32::EPSILON);
            assert_eq!(state, ContextThresholdState::Critical);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_skill_enabled_event() {
        let event = CodirigentEvent::SkillEnabled {
            name: "commit".to_string(),
        };
        if let CodirigentEvent::SkillEnabled { name } = event {
            assert_eq!(name, "commit");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_skill_disabled_event() {
        let event = CodirigentEvent::SkillDisabled {
            name: "commit".to_string(),
        };
        if let CodirigentEvent::SkillDisabled { name } = event {
            assert_eq!(name, "commit");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_token_budget_warning_event() {
        let budget = TokenBudget {
            max_tokens: 15000,
            used_tokens: 12500,
            warning_threshold: 12000,
        };
        let event = CodirigentEvent::TokenBudgetWarning { budget };
        if let CodirigentEvent::TokenBudgetWarning { budget } = event {
            assert_eq!(budget.used_tokens, 12500);
            assert!(budget.used_tokens >= budget.warning_threshold);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_token_budget_exceeded_event() {
        let budget = TokenBudget {
            max_tokens: 15000,
            used_tokens: 16000,
            warning_threshold: 12000,
        };
        let event = CodirigentEvent::TokenBudgetExceeded { budget };
        if let CodirigentEvent::TokenBudgetExceeded { budget } = event {
            assert_eq!(budget.used_tokens, 16000);
            assert!(budget.used_tokens > budget.max_tokens);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_skill_preset_applied_event() {
        let event = CodirigentEvent::SkillPresetApplied {
            preset_name: "dev".to_string(),
            enabled_count: 5,
        };
        if let CodirigentEvent::SkillPresetApplied {
            preset_name,
            enabled_count,
        } = event
        {
            assert_eq!(preset_name, "dev");
            assert_eq!(enabled_count, 5);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_skills_refreshed_event() {
        let event = CodirigentEvent::SkillsRefreshed { count: 10 };
        if let CodirigentEvent::SkillsRefreshed { count } = event {
            assert_eq!(count, 10);
        } else {
            panic!("Wrong event type");
        }
    }

    // === Ralph Loop Event Tests ===

    #[test]
    fn test_ralph_loop_started_event() {
        let event = CodirigentEvent::RalphLoopStarted {
            session_id: SessionId(1),
            config: crate::ralph::RalphLoopConfig::default(),
        };
        if let CodirigentEvent::RalphLoopStarted { session_id, config } = event {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(config.max_iterations, 20); // Default is 20
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_ralph_loop_paused_event() {
        let event = CodirigentEvent::RalphLoopPaused {
            session_id: SessionId(1),
        };
        if let CodirigentEvent::RalphLoopPaused { session_id } = event {
            assert_eq!(session_id, SessionId(1));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_ralph_loop_resumed_event() {
        let event = CodirigentEvent::RalphLoopResumed {
            session_id: SessionId(1),
        };
        if let CodirigentEvent::RalphLoopResumed { session_id } = event {
            assert_eq!(session_id, SessionId(1));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_ralph_loop_cancelled_event() {
        let event = CodirigentEvent::RalphLoopCancelled {
            session_id: SessionId(1),
        };
        if let CodirigentEvent::RalphLoopCancelled { session_id } = event {
            assert_eq!(session_id, SessionId(1));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_ralph_loop_iteration_event() {
        let event = CodirigentEvent::RalphLoopIteration {
            session_id: SessionId(1),
            iteration: 3,
            passed: true,
            test_failures: Some(0),
        };
        if let CodirigentEvent::RalphLoopIteration {
            session_id,
            iteration,
            passed,
            test_failures,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(iteration, 3);
            assert!(passed);
            assert_eq!(test_failures, Some(0));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_ralph_loop_completed_event() {
        let event = CodirigentEvent::RalphLoopCompleted {
            session_id: SessionId(1),
            total_iterations: 5,
        };
        if let CodirigentEvent::RalphLoopCompleted {
            session_id,
            total_iterations,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(total_iterations, 5);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_ralph_loop_failed_event() {
        let event = CodirigentEvent::RalphLoopFailed {
            session_id: SessionId(1),
            reason: "Max iterations reached".to_string(),
            iterations: 10,
        };
        if let CodirigentEvent::RalphLoopFailed {
            session_id,
            reason,
            iterations,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(reason, "Max iterations reached");
            assert_eq!(iterations, 10);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_ralph_loop_stuck_event() {
        let event = CodirigentEvent::RalphLoopStuck {
            session_id: SessionId(1),
            iterations_without_progress: 3,
        };
        if let CodirigentEvent::RalphLoopStuck {
            session_id,
            iterations_without_progress,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(iterations_without_progress, 3);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_ralph_loop_compacted_event() {
        let event = CodirigentEvent::RalphLoopCompacted {
            session_id: SessionId(1),
            iteration: 7,
        };
        if let CodirigentEvent::RalphLoopCompacted {
            session_id,
            iteration,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(iteration, 7);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_all_event_variants_clone() {
        // Test that all variants can be cloned
        let events: Vec<CodirigentEvent> = vec![
            CodirigentEvent::SessionCreated { id: SessionId(1) },
            CodirigentEvent::SessionClosed { id: SessionId(1) },
            CodirigentEvent::SessionStatusChanged {
                id: SessionId(1),
                old: SessionStatus::Idle,
                new: SessionStatus::Working,
            },
            CodirigentEvent::SessionOutputReceived {
                id: SessionId(1),
                data: vec![1, 2, 3],
            },
            CodirigentEvent::SessionRenamed {
                id: SessionId(1),
                old_name: "old".to_string(),
                new_name: "new".to_string(),
            },
            CodirigentEvent::SessionGroupChanged {
                id: SessionId(1),
                group: None,
                color: None,
            },
            CodirigentEvent::InputRequired {
                session_id: SessionId(1),
                pattern: None,
            },
            CodirigentEvent::InputProvided {
                session_id: SessionId(1),
            },
            CodirigentEvent::PermissionRequired {
                session_id: SessionId(1),
                tool_name: Some("Bash".to_string()),
            },
            CodirigentEvent::LayoutChanged {
                mode: LayoutMode::Single,
            },
            CodirigentEvent::SessionFocused { id: SessionId(1) },
            CodirigentEvent::TaskCreated {
                id: TaskId("t".to_string()),
            },
            CodirigentEvent::TaskAssigned {
                task_id: TaskId("t".to_string()),
                session_id: SessionId(1),
            },
            CodirigentEvent::TaskCompleted {
                task_id: TaskId("t".to_string()),
                success: true,
            },
            CodirigentEvent::PathDraggedToSession {
                session_id: SessionId(1),
                path: PathBuf::from("/tmp"),
            },
            CodirigentEvent::WorktreeCreated {
                path: PathBuf::from("/repo/worktrees/feature"),
                branch: "feature".to_string(),
            },
            CodirigentEvent::WorktreeRemoved {
                path: PathBuf::from("/repo/worktrees/feature"),
            },
            CodirigentEvent::SessionBoundToWorktree {
                session_id: SessionId(1),
                worktree_path: PathBuf::from("/repo/worktrees/feature"),
            },
            CodirigentEvent::SessionUnboundFromWorktree {
                session_id: SessionId(1),
            },
            CodirigentEvent::ContextUsageUpdated {
                session_id: SessionId(1),
                percentage: 0.5,
                effective_percentage: 0.625,
            },
            CodirigentEvent::ContextThresholdReached {
                session_id: SessionId(1),
                threshold: 0.7,
                state: ContextThresholdState::Warning,
            },
            CodirigentEvent::SkillEnabled {
                name: "commit".to_string(),
            },
            CodirigentEvent::SkillDisabled {
                name: "commit".to_string(),
            },
            CodirigentEvent::TokenBudgetWarning {
                budget: TokenBudget {
                    max_tokens: 15000,
                    used_tokens: 12500,
                    warning_threshold: 12000,
                },
            },
            CodirigentEvent::TokenBudgetExceeded {
                budget: TokenBudget {
                    max_tokens: 15000,
                    used_tokens: 16000,
                    warning_threshold: 12000,
                },
            },
            CodirigentEvent::SkillPresetApplied {
                preset_name: "dev".to_string(),
                enabled_count: 5,
            },
            CodirigentEvent::SkillsRefreshed { count: 10 },
            // Ralph Loop events
            CodirigentEvent::RalphLoopStarted {
                session_id: SessionId(1),
                config: crate::ralph::RalphLoopConfig::default(),
            },
            CodirigentEvent::RalphLoopPaused {
                session_id: SessionId(1),
            },
            CodirigentEvent::RalphLoopResumed {
                session_id: SessionId(1),
            },
            CodirigentEvent::RalphLoopCancelled {
                session_id: SessionId(1),
            },
            CodirigentEvent::RalphLoopIteration {
                session_id: SessionId(1),
                iteration: 1,
                passed: true,
                test_failures: Some(0),
            },
            CodirigentEvent::RalphLoopCompleted {
                session_id: SessionId(1),
                total_iterations: 5,
            },
            CodirigentEvent::RalphLoopFailed {
                session_id: SessionId(1),
                reason: "Max iterations reached".to_string(),
                iterations: 10,
            },
            CodirigentEvent::RalphLoopStuck {
                session_id: SessionId(1),
                iterations_without_progress: 3,
            },
            CodirigentEvent::RalphLoopCompacted {
                session_id: SessionId(1),
                iteration: 7,
            },
            CodirigentEvent::BroadcastSent {
                id: crate::broadcast::BroadcastId(1),
                target_count: 3,
                priority: crate::broadcast::BroadcastPriority::Normal,
            },
            CodirigentEvent::BroadcastDelivered {
                id: crate::broadcast::BroadcastId(1),
                session_id: SessionId(1),
            },
            CodirigentEvent::BroadcastDeliveryFailed {
                id: crate::broadcast::BroadcastId(1),
                session_id: SessionId(2),
                error: "Connection timeout".to_string(),
            },
            CodirigentEvent::BroadcastComplete {
                id: crate::broadcast::BroadcastId(1),
                success_count: 2,
                failure_count: 1,
            },
            // Working directory events
            CodirigentEvent::WorkingDirectoryChanged {
                id: SessionId(1),
                old_dir: PathBuf::from("/old/dir"),
                new_dir: PathBuf::from("/new/dir"),
            },
            // Git status events
            CodirigentEvent::GitStatusUpdated {
                session_id: SessionId(1),
                git_info: Some(crate::types::GitRepoInfo {
                    repo_root: PathBuf::from("/repo"),
                    branch: "main".to_string(),
                    dirty_count: 0,
                    has_staged: false,
                    head_sha: Some("abc12345".to_string()),
                    unstaged_files: vec![],
                    staged_files: vec![],
                }),
            },
            CodirigentEvent::GitStatusUpdated {
                session_id: SessionId(2),
                git_info: None,
            },
            // Clipboard events
            CodirigentEvent::ClipboardContentDetected {
                content_type: ClipboardContentType::Text,
            },
            CodirigentEvent::ClipboardImageSaved {
                session_id: Some(SessionId(1)),
                path: PathBuf::from("/tmp/image.png"),
            },
            CodirigentEvent::SmartPasteTriggered {
                session_id: SessionId(1),
                formatted_content: "content".to_string(),
            },
        ];

        for event in events {
            let _ = event.clone();
        }
    }

    #[test]
    fn test_broadcast_sent_event() {
        let event = CodirigentEvent::BroadcastSent {
            id: crate::broadcast::BroadcastId(1),
            target_count: 5,
            priority: crate::broadcast::BroadcastPriority::High,
        };
        if let CodirigentEvent::BroadcastSent {
            id,
            target_count,
            priority,
        } = event
        {
            assert_eq!(id, crate::broadcast::BroadcastId(1));
            assert_eq!(target_count, 5);
            assert_eq!(priority, crate::broadcast::BroadcastPriority::High);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_broadcast_delivered_event() {
        let event = CodirigentEvent::BroadcastDelivered {
            id: crate::broadcast::BroadcastId(1),
            session_id: SessionId(42),
        };
        if let CodirigentEvent::BroadcastDelivered { id, session_id } = event {
            assert_eq!(id, crate::broadcast::BroadcastId(1));
            assert_eq!(session_id, SessionId(42));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_broadcast_delivery_failed_event() {
        let event = CodirigentEvent::BroadcastDeliveryFailed {
            id: crate::broadcast::BroadcastId(1),
            session_id: SessionId(2),
            error: "Session offline".to_string(),
        };
        if let CodirigentEvent::BroadcastDeliveryFailed {
            id,
            session_id,
            error,
        } = event
        {
            assert_eq!(id, crate::broadcast::BroadcastId(1));
            assert_eq!(session_id, SessionId(2));
            assert_eq!(error, "Session offline");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_broadcast_complete_event() {
        let event = CodirigentEvent::BroadcastComplete {
            id: crate::broadcast::BroadcastId(1),
            success_count: 8,
            failure_count: 2,
        };
        if let CodirigentEvent::BroadcastComplete {
            id,
            success_count,
            failure_count,
        } = event
        {
            assert_eq!(id, crate::broadcast::BroadcastId(1));
            assert_eq!(success_count, 8);
            assert_eq!(failure_count, 2);
        } else {
            panic!("Wrong event type");
        }
    }

    // === Advanced Session Event Tests ===

    #[test]
    fn test_handoff_initiated_event() {
        let event = CodirigentEvent::HandoffInitiated {
            source: SessionId(1),
            target: SessionId(2),
        };
        if let CodirigentEvent::HandoffInitiated { source, target } = event {
            assert_eq!(source, SessionId(1));
            assert_eq!(target, SessionId(2));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_handoff_completed_event() {
        let event = CodirigentEvent::HandoffCompleted {
            source: SessionId(1),
            target: SessionId(2),
        };
        if let CodirigentEvent::HandoffCompleted { source, target } = event {
            assert_eq!(source, SessionId(1));
            assert_eq!(target, SessionId(2));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_handoff_failed_event() {
        let event = CodirigentEvent::HandoffFailed {
            source: SessionId(1),
            target: SessionId(2),
            error: "Target session busy".to_string(),
        };
        if let CodirigentEvent::HandoffFailed {
            source,
            target,
            error,
        } = event
        {
            assert_eq!(source, SessionId(1));
            assert_eq!(target, SessionId(2));
            assert_eq!(error, "Target session busy");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_created_from_template_event() {
        let event = CodirigentEvent::SessionCreatedFromTemplate {
            session_id: SessionId(1),
            template_name: "development".to_string(),
        };
        if let CodirigentEvent::SessionCreatedFromTemplate {
            session_id,
            template_name,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(template_name, "development");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_template_saved_event() {
        let event = CodirigentEvent::TemplateSaved {
            name: "my-template".to_string(),
        };
        if let CodirigentEvent::TemplateSaved { name } = event {
            assert_eq!(name, "my-template");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_cloned_event() {
        let event = CodirigentEvent::SessionCloned {
            source_session: SessionId(1),
            cloned_session: SessionId(2),
        };
        if let CodirigentEvent::SessionCloned {
            source_session,
            cloned_session,
        } = event
        {
            assert_eq!(source_session, SessionId(1));
            assert_eq!(cloned_session, SessionId(2));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_group_created_event() {
        let event = CodirigentEvent::SessionGroupCreated {
            name: "backend".to_string(),
        };
        if let CodirigentEvent::SessionGroupCreated { name } = event {
            assert_eq!(name, "backend");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_added_to_group_event() {
        let event = CodirigentEvent::SessionAddedToGroup {
            session_id: SessionId(1),
            group_name: "backend".to_string(),
        };
        if let CodirigentEvent::SessionAddedToGroup {
            session_id,
            group_name,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(group_name, "backend");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_session_removed_from_group_event() {
        let event = CodirigentEvent::SessionRemovedFromGroup {
            session_id: SessionId(1),
            group_name: "backend".to_string(),
        };
        if let CodirigentEvent::SessionRemovedFromGroup {
            session_id,
            group_name,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(group_name, "backend");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_overnight_started_event() {
        let config = OvernightConfig::default();
        let event = CodirigentEvent::OvernightStarted {
            config: config.clone(),
        };
        if let CodirigentEvent::OvernightStarted { config: c } = event {
            assert_eq!(c.start_hour, config.start_hour);
            assert_eq!(c.stop_hour, config.stop_hour);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_overnight_stopped_event() {
        let summary = OvernightSummary::default();
        let event = CodirigentEvent::OvernightStopped {
            summary: summary.clone(),
        };
        if let CodirigentEvent::OvernightStopped { summary: s } = event {
            assert_eq!(s.tasks_completed, summary.tasks_completed);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_overnight_task_completed_event() {
        let event = CodirigentEvent::OvernightTaskCompleted {
            task_number: 5,
            success: true,
        };
        if let CodirigentEvent::OvernightTaskCompleted {
            task_number,
            success,
        } = event
        {
            assert_eq!(task_number, 5);
            assert!(success);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_overnight_error_event() {
        let event = CodirigentEvent::OvernightError {
            error: "Task timeout".to_string(),
            task_number: Some(3),
        };
        if let CodirigentEvent::OvernightError { error, task_number } = event {
            assert_eq!(error, "Task timeout");
            assert_eq!(task_number, Some(3));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_overnight_error_event_no_task() {
        let event = CodirigentEvent::OvernightError {
            error: "System error".to_string(),
            task_number: None,
        };
        if let CodirigentEvent::OvernightError { error, task_number } = event {
            assert_eq!(error, "System error");
            assert!(task_number.is_none());
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_advanced_session_events_clone() {
        // Test that all advanced session event variants can be cloned
        let events: Vec<CodirigentEvent> = vec![
            CodirigentEvent::HandoffInitiated {
                source: SessionId(1),
                target: SessionId(2),
            },
            CodirigentEvent::HandoffCompleted {
                source: SessionId(1),
                target: SessionId(2),
            },
            CodirigentEvent::HandoffFailed {
                source: SessionId(1),
                target: SessionId(2),
                error: "error".to_string(),
            },
            CodirigentEvent::SessionCreatedFromTemplate {
                session_id: SessionId(1),
                template_name: "dev".to_string(),
            },
            CodirigentEvent::TemplateSaved {
                name: "template".to_string(),
            },
            CodirigentEvent::SessionCloned {
                source_session: SessionId(1),
                cloned_session: SessionId(2),
            },
            CodirigentEvent::SessionGroupCreated {
                name: "group".to_string(),
            },
            CodirigentEvent::SessionAddedToGroup {
                session_id: SessionId(1),
                group_name: "group".to_string(),
            },
            CodirigentEvent::SessionRemovedFromGroup {
                session_id: SessionId(1),
                group_name: "group".to_string(),
            },
            CodirigentEvent::OvernightStarted {
                config: OvernightConfig::default(),
            },
            CodirigentEvent::OvernightStopped {
                summary: OvernightSummary::default(),
            },
            CodirigentEvent::OvernightTaskCompleted {
                task_number: 1,
                success: true,
            },
            CodirigentEvent::OvernightError {
                error: "err".to_string(),
                task_number: None,
            },
        ];

        for event in events {
            let _ = event.clone();
        }
    }

    // === Git Status Event Tests ===

    #[test]
    fn test_git_status_updated_event_with_info() {
        let git_info = crate::types::GitRepoInfo {
            repo_root: PathBuf::from("/home/user/project"),
            branch: "feature-branch".to_string(),
            dirty_count: 5,
            has_staged: true,
            head_sha: Some("abc12345".to_string()),
            unstaged_files: vec![],
            staged_files: vec![],
        };
        let event = CodirigentEvent::GitStatusUpdated {
            session_id: SessionId(1),
            git_info: Some(git_info),
        };
        if let CodirigentEvent::GitStatusUpdated {
            session_id,
            git_info,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            let info = git_info.unwrap();
            assert_eq!(info.branch, "feature-branch");
            assert_eq!(info.dirty_count, 5);
            assert!(info.has_staged);
            assert_eq!(info.head_sha, Some("abc12345".to_string()));
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_git_status_updated_event_none() {
        let event = CodirigentEvent::GitStatusUpdated {
            session_id: SessionId(1),
            git_info: None,
        };
        if let CodirigentEvent::GitStatusUpdated {
            session_id,
            git_info,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert!(git_info.is_none());
        } else {
            panic!("Wrong event type");
        }
    }

    // === Clipboard Event Tests ===

    #[test]
    fn test_clipboard_content_type_variants() {
        // Test all variants
        let text = ClipboardContentType::Text;
        let image = ClipboardContentType::Image;
        let files = ClipboardContentType::Files;
        let empty = ClipboardContentType::Empty;

        // Test equality
        assert_eq!(text, ClipboardContentType::Text);
        assert_eq!(image, ClipboardContentType::Image);
        assert_eq!(files, ClipboardContentType::Files);
        assert_eq!(empty, ClipboardContentType::Empty);

        // Test inequality
        assert_ne!(text, image);
        assert_ne!(image, files);
        assert_ne!(files, empty);

        // Test debug
        assert!(format!("{:?}", text).contains("Text"));
        assert!(format!("{:?}", image).contains("Image"));
        assert!(format!("{:?}", files).contains("Files"));
        assert!(format!("{:?}", empty).contains("Empty"));

        // Test copy
        let text_copy = text;
        assert_eq!(text_copy, ClipboardContentType::Text);

        // Test clone
        let text_cloned = text;
        assert_eq!(text_cloned, ClipboardContentType::Text);
    }

    #[test]
    fn test_clipboard_content_detected_event() {
        let event = CodirigentEvent::ClipboardContentDetected {
            content_type: ClipboardContentType::Text,
        };
        if let CodirigentEvent::ClipboardContentDetected { content_type } = event {
            assert_eq!(content_type, ClipboardContentType::Text);
        } else {
            panic!("Wrong event type");
        }

        // Test with image type
        let event = CodirigentEvent::ClipboardContentDetected {
            content_type: ClipboardContentType::Image,
        };
        if let CodirigentEvent::ClipboardContentDetected { content_type } = event {
            assert_eq!(content_type, ClipboardContentType::Image);
        } else {
            panic!("Wrong event type");
        }

        // Test with files type
        let event = CodirigentEvent::ClipboardContentDetected {
            content_type: ClipboardContentType::Files,
        };
        if let CodirigentEvent::ClipboardContentDetected { content_type } = event {
            assert_eq!(content_type, ClipboardContentType::Files);
        } else {
            panic!("Wrong event type");
        }

        // Test with empty type
        let event = CodirigentEvent::ClipboardContentDetected {
            content_type: ClipboardContentType::Empty,
        };
        if let CodirigentEvent::ClipboardContentDetected { content_type } = event {
            assert_eq!(content_type, ClipboardContentType::Empty);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_clipboard_image_saved_event() {
        // Test with session ID
        let event = CodirigentEvent::ClipboardImageSaved {
            session_id: Some(SessionId(42)),
            path: PathBuf::from("/tmp/clipboard_image.png"),
        };
        if let CodirigentEvent::ClipboardImageSaved { session_id, path } = event {
            assert_eq!(session_id, Some(SessionId(42)));
            assert_eq!(path, PathBuf::from("/tmp/clipboard_image.png"));
        } else {
            panic!("Wrong event type");
        }

        // Test without session ID (no focused session)
        let event = CodirigentEvent::ClipboardImageSaved {
            session_id: None,
            path: PathBuf::from("/project/.codirigent/images/img_001.png"),
        };
        if let CodirigentEvent::ClipboardImageSaved { session_id, path } = event {
            assert!(session_id.is_none());
            assert_eq!(
                path,
                PathBuf::from("/project/.codirigent/images/img_001.png")
            );
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_smart_paste_triggered_event() {
        let event = CodirigentEvent::SmartPasteTriggered {
            session_id: SessionId(1),
            formatted_content: "formatted code block".to_string(),
        };
        if let CodirigentEvent::SmartPasteTriggered {
            session_id,
            formatted_content,
        } = event
        {
            assert_eq!(session_id, SessionId(1));
            assert_eq!(formatted_content, "formatted code block");
        } else {
            panic!("Wrong event type");
        }

        // Test with empty content
        let event = CodirigentEvent::SmartPasteTriggered {
            session_id: SessionId(99),
            formatted_content: String::new(),
        };
        if let CodirigentEvent::SmartPasteTriggered {
            session_id,
            formatted_content,
        } = event
        {
            assert_eq!(session_id, SessionId(99));
            assert!(formatted_content.is_empty());
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_clipboard_events_clone() {
        // Test that all clipboard event variants can be cloned
        let events: Vec<CodirigentEvent> = vec![
            CodirigentEvent::ClipboardContentDetected {
                content_type: ClipboardContentType::Text,
            },
            CodirigentEvent::ClipboardContentDetected {
                content_type: ClipboardContentType::Image,
            },
            CodirigentEvent::ClipboardContentDetected {
                content_type: ClipboardContentType::Files,
            },
            CodirigentEvent::ClipboardContentDetected {
                content_type: ClipboardContentType::Empty,
            },
            CodirigentEvent::ClipboardImageSaved {
                session_id: Some(SessionId(1)),
                path: PathBuf::from("/tmp/image.png"),
            },
            CodirigentEvent::ClipboardImageSaved {
                session_id: None,
                path: PathBuf::from("/tmp/image.png"),
            },
            CodirigentEvent::SmartPasteTriggered {
                session_id: SessionId(1),
                formatted_content: "content".to_string(),
            },
        ];

        for event in events {
            let cloned = event.clone();
            // Verify the clone worked by checking debug output
            let _ = format!("{:?}", cloned);
        }
    }
}
