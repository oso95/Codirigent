//! Status and state enums for sessions and tasks.

use serde::{Deserialize, Serialize};

/// Session status detected by the Input Detector module.
///
/// This represents the current state of a session as determined
/// by process monitoring and output pattern detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SessionStatus {
    /// No active process, shell is idle (or process terminated).
    #[default]
    Idle,
    /// Process is actively running (CPU activity detected).
    Working,
    /// Session needs user attention (input required or permission prompt).
    NeedsAttention,
    /// An agent has finished responding and is waiting for the next user prompt.
    /// Produced by hooks, structured logs, or terminal-screen semantic rules.
    /// Cleared to Idle when the user switches to this session.
    ResponseReady,
    /// Error detected in output.
    Error,
}

/// Shell command lifecycle state from OSC 133 (FinalTerm protocol) markers.
///
/// Modern shells emit these markers to signal prompt/command lifecycle,
/// enabling reliable idle detection without process-state heuristics.
///
/// # State Mapping
///
/// - `PromptStart` / `CommandInputStart` → shell is idle
/// - `CommandExecuted` → command is running
/// - `CommandFinished` → command done (brief transition before next prompt)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellState {
    /// Prompt is being displayed (133;A) — shell is idle.
    PromptStart,
    /// User can type a command (133;B) — still idle.
    CommandInputStart,
    /// Command is executing (133;C) — working.
    CommandExecuted,
    /// Command finished (133;D) with optional exit code.
    CommandFinished {
        /// The exit code of the finished command, if reported.
        exit_code: Option<i32>,
    },
}

/// Context threshold state for context window tracking.
///
/// Represents the current state relative to configured thresholds.
///
/// # Example
///
/// ```
/// use codirigent_core::ContextThresholdState;
///
/// assert_eq!(ContextThresholdState::default(), ContextThresholdState::Normal);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ContextThresholdState {
    /// Below warning threshold.
    #[default]
    Normal,
    /// At or above warning threshold, below critical.
    Warning,
    /// At or above critical threshold.
    Critical,
}

/// Task priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskPriority {
    /// Critical priority - must be done first.
    Critical,
    /// High priority.
    High,
    /// Medium priority (default).
    #[default]
    Medium,
    /// Low priority.
    Low,
}

/// Task status in the workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskStatus {
    /// Waiting in queue.
    #[default]
    Queued,
    /// Assigned to a session.
    Assigned,
    /// Currently being worked on.
    Working,
    /// Awaiting verification.
    Verifying,
    /// Ready for human review.
    Review,
    /// Completed successfully.
    Done,
    /// Blocked by dependency or error.
    Blocked,
}
