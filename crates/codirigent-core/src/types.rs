//! Core types for the Codirigent application.
//!
//! This module contains all shared types used throughout the application,
//! including identifiers, enums, and core data structures.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Unique identifier for a session.
///
/// Sessions are the core unit of work in Codirigent, each representing
/// a terminal instance running an AI CLI tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u64);

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session-{}", self.0)
    }
}

/// Unique identifier for a task.
///
/// Tasks are work items that can be assigned to sessions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

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

/// Verification configuration for a task.
///
/// Defines how to verify task completion (run tests, custom scripts, etc.)
/// When a task has verification configured, it will transition to the
/// `Verifying` status after the AI completes its work.
///
/// # Example
///
/// ```
/// use codirigent_core::VerificationConfig;
/// use std::time::Duration;
///
/// let config = VerificationConfig {
///     command: "cargo test".to_string(),
///     working_dir: None,
///     timeout: Duration::from_secs(300),
///     requires_human_review: true,
///     success_patterns: vec!["test result: ok".to_string()],
///     failure_patterns: vec!["FAILED".to_string()],
/// };
/// assert_eq!(config.timeout, Duration::from_secs(300));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationConfig {
    /// Command to execute for verification (e.g., "npm test", "cargo test").
    pub command: String,

    /// Working directory for the command. If None, uses task's assigned session's directory.
    pub working_dir: Option<PathBuf>,

    /// Timeout for verification command.
    #[serde(with = "humantime_serde")]
    pub timeout: Duration,

    /// Whether human review is required after verification passes.
    pub requires_human_review: bool,

    /// Custom success patterns to look for in output.
    pub success_patterns: Vec<String>,

    /// Custom failure patterns that indicate test failure.
    pub failure_patterns: Vec<String>,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            working_dir: None,
            timeout: Duration::from_secs(300), // 5 minutes
            requires_human_review: true,
            success_patterns: Vec::new(),
            failure_patterns: Vec::new(),
        }
    }
}

/// Retry configuration for a task.
///
/// Controls how many times a task can be retried if it fails,
/// and the delay between retry attempts.
///
/// # Example
///
/// ```
/// use codirigent_core::RetryConfig;
/// use std::time::Duration;
///
/// let mut config = RetryConfig::default();
/// assert_eq!(config.max_retries, 3);
/// assert_eq!(config.retry_count, 0);
///
/// // Simulate retries
/// config.retry_count = 2;
/// assert!(config.retry_count < config.max_retries);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,

    /// Current retry count.
    pub retry_count: u32,

    /// Delay between retries.
    #[serde(with = "humantime_serde")]
    pub retry_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_count: 0,
            retry_delay: Duration::from_secs(0),
        }
    }
}

/// Grid position for custom layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GridPosition {
    /// Row index (0-based).
    pub row: u32,
    /// Column index (0-based).
    pub col: u32,
}

/// Unique identifier for a layout slot in a split tree.
///
/// Slots decouple tree shape from session lifecycle — empty slots are valid,
/// and sessions can be reassigned between slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlotId(pub u32);

impl std::fmt::Display for SlotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "slot-{}", self.0)
    }
}

/// Direction of a binary split in the layout tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    /// Children arranged left-to-right.
    Horizontal,
    /// Children arranged top-to-bottom.
    Vertical,
}

/// A node in the binary split layout tree.
///
/// Binary splits are simpler than n-ary splits (one drag handle per split),
/// can represent any layout via nesting, and match the approach of
/// tmux/iTerm2/VS Code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutNode {
    /// An internal split node dividing space between two children.
    Split {
        /// Direction of the split.
        direction: SplitDirection,
        /// Ratio (0.0..1.0) — first child's share of available space.
        ratio: f32,
        /// First child (left or top).
        first: Box<LayoutNode>,
        /// Second child (right or bottom).
        second: Box<LayoutNode>,
    },
    /// A leaf node representing a single pane slot.
    Leaf {
        /// The slot identifier for this pane.
        slot: SlotId,
    },
}

impl LayoutNode {
    /// Count the number of leaf nodes in this tree.
    pub fn leaf_count(&self) -> usize {
        match self {
            LayoutNode::Leaf { .. } => 1,
            LayoutNode::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    /// DFS traversal returning slots in left-to-right, top-to-bottom order.
    pub fn slots_in_order(&self) -> Vec<SlotId> {
        let mut result = Vec::new();
        self.collect_slots(&mut result);
        result
    }

    fn collect_slots(&self, out: &mut Vec<SlotId>) {
        match self {
            LayoutNode::Leaf { slot } => out.push(*slot),
            LayoutNode::Split { first, second, .. } => {
                first.collect_slots(out);
                second.collect_slots(out);
            }
        }
    }

    /// Convert a grid (rows x cols) to an equivalent split tree.
    ///
    /// A 2x3 grid becomes:
    /// ```text
    ///   V(0.5)
    ///   ├── H(0.333) → H(0.5) → [slot0] [slot1] [slot2]
    ///   └── H(0.333) → H(0.5) → [slot3] [slot4] [slot5]
    /// ```
    pub fn from_grid(rows: u32, cols: u32) -> Self {
        assert!(rows >= 1 && cols >= 1, "Grid must have at least 1 row and 1 column");
        let mut next_slot = 0u32;
        Self::build_grid_rows(rows, cols, &mut next_slot)
    }

    fn build_grid_rows(rows: u32, cols: u32, next_slot: &mut u32) -> Self {
        if rows == 1 {
            return Self::build_grid_cols(cols, next_slot);
        }
        let first = Self::build_grid_cols(cols, next_slot);
        let second = Self::build_grid_rows(rows - 1, cols, next_slot);
        LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio: 1.0 / rows as f32,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    fn build_grid_cols(cols: u32, next_slot: &mut u32) -> Self {
        if cols == 1 {
            let slot = SlotId(*next_slot);
            *next_slot += 1;
            return LayoutNode::Leaf { slot };
        }
        let slot = SlotId(*next_slot);
        *next_slot += 1;
        let first = LayoutNode::Leaf { slot };
        let second = Self::build_grid_cols(cols - 1, next_slot);
        LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 1.0 / cols as f32,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Split a leaf node into two new leaves.
    ///
    /// Returns `(new_tree, new_slot_id)` where `new_slot_id` is the second child's slot.
    /// The original slot keeps the first child position.
    /// Returns `None` if the target slot is not found.
    pub fn split_slot(
        &self,
        target: SlotId,
        direction: SplitDirection,
        ratio: f32,
        new_slot: SlotId,
    ) -> Option<LayoutNode> {
        match self {
            LayoutNode::Leaf { slot } if *slot == target => {
                Some(LayoutNode::Split {
                    direction,
                    ratio,
                    first: Box::new(LayoutNode::Leaf { slot: *slot }),
                    second: Box::new(LayoutNode::Leaf { slot: new_slot }),
                })
            }
            LayoutNode::Leaf { .. } => None,
            LayoutNode::Split {
                direction: d,
                ratio: r,
                first,
                second,
            } => {
                if let Some(new_first) = first.split_slot(target, direction, ratio, new_slot) {
                    Some(LayoutNode::Split {
                        direction: *d,
                        ratio: *r,
                        first: Box::new(new_first),
                        second: second.clone(),
                    })
                } else if let Some(new_second) =
                    second.split_slot(target, direction, ratio, new_slot)
                {
                    Some(LayoutNode::Split {
                        direction: *d,
                        ratio: *r,
                        first: first.clone(),
                        second: Box::new(new_second),
                    })
                } else {
                    None
                }
            }
        }
    }

    /// Remove a leaf node and promote its sibling.
    ///
    /// Returns `None` if the target slot is not found or if this is the root leaf.
    pub fn close_slot(&self, target: SlotId) -> Option<LayoutNode> {
        match self {
            LayoutNode::Leaf { .. } => {
                // Can't close the root leaf
                None
            }
            LayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // Check if first child is the target leaf
                if let LayoutNode::Leaf { slot } = first.as_ref() {
                    if *slot == target {
                        return Some(second.as_ref().clone());
                    }
                }
                // Check if second child is the target leaf
                if let LayoutNode::Leaf { slot } = second.as_ref() {
                    if *slot == target {
                        return Some(first.as_ref().clone());
                    }
                }
                // Recurse into children
                if let Some(new_first) = first.close_slot(target) {
                    Some(LayoutNode::Split {
                        direction: *direction,
                        ratio: *ratio,
                        first: Box::new(new_first),
                        second: second.clone(),
                    })
                } else if let Some(new_second) = second.close_slot(target) {
                    Some(LayoutNode::Split {
                        direction: *direction,
                        ratio: *ratio,
                        first: first.clone(),
                        second: Box::new(new_second),
                    })
                } else {
                    None
                }
            }
        }
    }

    /// Adjust the split ratio for the parent of a given slot.
    ///
    /// Finds the split node that directly contains the target slot as a child
    /// and updates its ratio. Returns `None` if the slot is not found or is the root leaf.
    pub fn set_ratio_for_slot(&self, target: SlotId, new_ratio: f32) -> Option<LayoutNode> {
        let clamped = new_ratio.clamp(0.1, 0.9);
        match self {
            LayoutNode::Leaf { .. } => None,
            LayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // Check if target is a direct child
                let first_contains = self.direct_child_has_slot(first, target);
                let second_contains = self.direct_child_has_slot(second, target);

                if first_contains || second_contains {
                    Some(LayoutNode::Split {
                        direction: *direction,
                        ratio: clamped,
                        first: first.clone(),
                        second: second.clone(),
                    })
                } else {
                    // Recurse
                    if let Some(new_first) = first.set_ratio_for_slot(target, new_ratio) {
                        Some(LayoutNode::Split {
                            direction: *direction,
                            ratio: *ratio,
                            first: Box::new(new_first),
                            second: second.clone(),
                        })
                    } else if let Some(new_second) =
                        second.set_ratio_for_slot(target, new_ratio)
                    {
                        Some(LayoutNode::Split {
                            direction: *direction,
                            ratio: *ratio,
                            first: first.clone(),
                            second: Box::new(new_second),
                        })
                    } else {
                        None
                    }
                }
            }
        }
    }

    fn direct_child_has_slot(&self, child: &LayoutNode, target: SlotId) -> bool {
        matches!(child, LayoutNode::Leaf { slot } if *slot == target)
    }

    /// Check if this tree contains a specific slot.
    pub fn contains_slot(&self, target: SlotId) -> bool {
        match self {
            LayoutNode::Leaf { slot } => *slot == target,
            LayoutNode::Split { first, second, .. } => {
                first.contains_slot(target) || second.contains_slot(target)
            }
        }
    }
}

/// Layout mode for the workspace grid.
///
/// Supports standard grid configurations, single-pane mode,
/// custom layouts with explicit positioning, and binary split trees.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LayoutMode {
    /// Standard grid layout with specified rows and columns.
    /// Common configurations: 2x2, 1x4, 2x3, 3x3.
    Grid {
        /// Number of rows.
        rows: u32,
        /// Number of columns.
        cols: u32,
    },
    /// Single session takes full window.
    Single,
    /// Custom layout with explicit session positions.
    Custom {
        /// Session positions.
        positions: Vec<(SessionId, GridPosition)>,
    },
    /// Binary split tree layout for arbitrary asymmetric pane arrangements.
    SplitTree {
        /// Root node of the split tree.
        root: LayoutNode,
    },
}

impl Default for LayoutMode {
    fn default() -> Self {
        LayoutMode::Grid { rows: 2, cols: 2 }
    }
}

/// Git repository information for a session's working directory.
///
/// Contains branch name, dirty file count, staged status, and HEAD SHA.
/// Populated by the git status service when the session's working directory
/// is inside a git repository.
///
/// # Example
///
/// ```
/// use codirigent_core::GitRepoInfo;
/// use std::path::PathBuf;
///
/// let info = GitRepoInfo {
///     repo_root: PathBuf::from("/home/user/project"),
///     branch: "main".to_string(),
///     dirty_count: 3,
///     has_staged: true,
///     head_sha: Some("abc12345".to_string()),
///     unstaged_files: vec![],
///     staged_files: vec![],
/// };
/// assert_eq!(info.branch, "main");
/// assert_eq!(info.dirty_count, 3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitRepoInfo {
    /// Absolute path to the git repository root.
    pub repo_root: PathBuf,
    /// Current branch name (or "HEAD detached" if detached).
    pub branch: String,
    /// Number of modified + untracked files.
    pub dirty_count: usize,
    /// Whether there are any staged changes.
    pub has_staged: bool,
    /// Short HEAD SHA (8 characters), if available.
    pub head_sha: Option<String>,
    /// Files with unstaged changes (working tree).
    pub unstaged_files: Vec<GitChangedFile>,
    /// Files with staged changes (index).
    pub staged_files: Vec<GitChangedFile>,
}

/// A file with changes in the git repository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitChangedFile {
    /// Relative path from repo root.
    pub path: String,
    /// Type of change.
    pub change: GitChangeKind,
}

/// Kind of change for a git file.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GitChangeKind {
    /// File content modified.
    Modified,
    /// New file (untracked or added).
    Added,
    /// File deleted.
    Deleted,
    /// File renamed.
    Renamed,
}

/// Session metadata and state.
///
/// This is the persistent representation of a session,
/// stored in state.json and used throughout the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier.
    pub id: SessionId,
    /// Human-readable session name.
    pub name: String,
    /// Current session status.
    pub status: SessionStatus,
    /// Working directory for this session.
    pub working_directory: PathBuf,
    /// Currently assigned task, if any.
    pub current_task: Option<TaskId>,
    /// Context window usage (0.0 - 1.0), if available.
    pub context_usage: Option<f32>,
    /// When the session was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Session group name for visual grouping.
    pub group: Option<String>,
    /// Group color for visual identification.
    pub color: Option<String>,
    /// Git repository information (branch, dirty count, etc.).
    pub git_info: Option<GitRepoInfo>,
}

impl Session {
    /// Create a new session with default values.
    pub fn new(id: SessionId, name: String, working_directory: PathBuf) -> Self {
        Self {
            id,
            name,
            status: SessionStatus::default(),
            working_directory,
            current_task: None,
            context_usage: None,
            created_at: chrono::Utc::now(),
            group: None,
            color: None,
            git_info: None,
        }
    }
}

/// Task definition for the task queue.
///
/// A task represents a unit of work that can be assigned to an AI session.
/// Tasks support verification (running tests), retry logic, and dependency
/// tracking for proper ordering.
///
/// # Example
///
/// ```
/// use codirigent_core::{Task, TaskId, VerificationConfig, RetryConfig};
///
/// let mut task = Task::new(
///     TaskId("task-001".to_string()),
///     "Implement login".to_string(),
///     "Add user authentication".to_string(),
/// );
///
/// // Add verification
/// task.verification = Some(VerificationConfig {
///     command: "cargo test auth".to_string(),
///     ..Default::default()
/// });
///
/// assert!(task.can_retry());
/// assert!(task.dependencies_satisfied(&[]));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier.
    pub id: TaskId,
    /// Task title (short description).
    pub title: String,
    /// Detailed description/instructions.
    pub description: String,
    /// Priority level.
    pub priority: TaskPriority,
    /// Current status in workflow.
    pub status: TaskStatus,
    /// Dependencies on other tasks (must complete first).
    pub dependencies: Vec<TaskId>,
    /// Tags for categorization and filtering.
    pub tags: Vec<String>,
    /// Estimated time to complete in minutes.
    pub estimated_minutes: Option<u32>,
    /// Assigned session, if any.
    pub assigned_session: Option<SessionId>,
    /// When assigned to a session.
    pub assigned_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Verification configuration.
    pub verification: Option<VerificationConfig>,
    /// Retry configuration.
    pub retry: RetryConfig,
    /// When the task was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When work started (status changed to Working).
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// When the task was completed.
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Error message if task failed.
    pub error_message: Option<String>,

    /// Project root directory for hard session filtering.
    /// When set, only sessions whose working_directory is under this path can be assigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<PathBuf>,

    /// Plan file path relative to project_dir.
    /// Included in assignment prompt so the CLI LLM can read it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_file: Option<String>,
}

impl Task {
    /// Create a new task with default values.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique task identifier
    /// * `title` - Short task title
    /// * `description` - Detailed instructions
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{Task, TaskId, TaskStatus};
    ///
    /// let task = Task::new(
    ///     TaskId("task-001".to_string()),
    ///     "Test Task".to_string(),
    ///     "Do something".to_string(),
    /// );
    /// assert_eq!(task.status, TaskStatus::Queued);
    /// assert!(task.can_retry());
    /// ```
    pub fn new(id: TaskId, title: String, description: String) -> Self {
        Self {
            id,
            title,
            description,
            priority: TaskPriority::default(),
            status: TaskStatus::default(),
            dependencies: Vec::new(),
            tags: Vec::new(),
            estimated_minutes: None,
            assigned_session: None,
            assigned_at: None,
            verification: None,
            retry: RetryConfig::default(),
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            error_message: None,
            project_dir: None,
            plan_file: None,
        }
    }

    /// Check if all dependencies are satisfied.
    ///
    /// Returns true if all tasks in the dependencies list are present
    /// in the completed_tasks list.
    ///
    /// # Arguments
    ///
    /// * `completed_tasks` - List of task IDs that have been completed
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{Task, TaskId};
    ///
    /// let mut task = Task::new(
    ///     TaskId("task-002".to_string()),
    ///     "Has deps".to_string(),
    ///     "".to_string(),
    /// );
    /// task.dependencies = vec![TaskId("task-001".to_string())];
    ///
    /// assert!(!task.dependencies_satisfied(&[]));
    /// assert!(task.dependencies_satisfied(&[TaskId("task-001".to_string())]));
    /// ```
    pub fn dependencies_satisfied(&self, completed_tasks: &[TaskId]) -> bool {
        self.dependencies
            .iter()
            .all(|dep| completed_tasks.contains(dep))
    }

    /// Check if the task can be retried.
    ///
    /// Returns true if the current retry count is less than the maximum
    /// allowed retries.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{Task, TaskId};
    ///
    /// let mut task = Task::new(
    ///     TaskId("task-001".to_string()),
    ///     "Retry Task".to_string(),
    ///     "".to_string(),
    /// );
    /// assert!(task.can_retry()); // Default: 0 retries out of 3
    ///
    /// task.retry.retry_count = 3;
    /// assert!(!task.can_retry()); // 3 retries out of 3, no more allowed
    /// ```
    pub fn can_retry(&self) -> bool {
        self.retry.retry_count < self.retry.max_retries
    }

    /// Increment the retry count.
    ///
    /// This should be called when a task fails and is being retried.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{Task, TaskId};
    ///
    /// let mut task = Task::new(
    ///     TaskId("task-001".to_string()),
    ///     "Retry Task".to_string(),
    ///     "".to_string(),
    /// );
    /// assert_eq!(task.retry.retry_count, 0);
    ///
    /// task.increment_retry();
    /// assert_eq!(task.retry.retry_count, 1);
    /// ```
    pub fn increment_retry(&mut self) {
        self.retry.retry_count += 1;
    }
}

/// Application state persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    /// All active sessions.
    pub sessions: Vec<Session>,
    /// Current layout mode.
    pub layout: LayoutMode,
    /// Last updated timestamp.
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Queue state persisted to queue.json.
///
/// Tracks the ordered list of queued tasks and blocked task dependencies.
/// This is used by the task scheduler to determine which tasks can be
/// assigned to sessions.
///
/// # Example
///
/// ```
/// use codirigent_core::{QueueState, TaskId};
///
/// let mut state = QueueState::default();
/// state.order.push(TaskId("task-001".to_string()));
/// state.order.push(TaskId("task-002".to_string()));
///
/// // task-003 is blocked by task-001
/// state.blocked.insert(
///     TaskId("task-003".to_string()),
///     vec![TaskId("task-001".to_string())],
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct QueueState {
    /// Ordered list of queued task IDs (priority order).
    pub order: Vec<TaskId>,

    /// Map of blocked task ID to blocking task IDs.
    pub blocked: HashMap<TaskId, Vec<TaskId>>,

    /// Last updated timestamp.
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Verification result from running tests.
///
/// Captures the output and results from running a verification command
/// (typically a test suite) for a task.
///
/// # Example
///
/// ```
/// use codirigent_core::{VerificationResult, TestResults};
/// use std::time::Duration;
///
/// let result = VerificationResult {
///     success: true,
///     exit_code: Some(0),
///     stdout: "All tests passed".to_string(),
///     stderr: "".to_string(),
///     test_results: None,
///     duration: Duration::from_secs(5),
///     run_at: chrono::Utc::now(),
/// };
/// assert!(result.success);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationResult {
    /// Whether verification passed.
    pub success: bool,

    /// Exit code of the verification command.
    pub exit_code: Option<i32>,

    /// Standard output from the command.
    pub stdout: String,

    /// Standard error from the command.
    pub stderr: String,

    /// Parsed test results if available.
    pub test_results: Option<TestResults>,

    /// Duration of the verification run.
    #[serde(with = "humantime_serde")]
    pub duration: Duration,

    /// When verification was run.
    pub run_at: chrono::DateTime<chrono::Utc>,
}

/// Parsed test results from verification output.
///
/// Contains aggregate counts and individual failure details
/// extracted from the test runner output.
///
/// # Example
///
/// ```
/// use codirigent_core::TestResults;
///
/// let results = TestResults {
///     total: 10,
///     passed: 8,
///     failed: 2,
///     skipped: 0,
///     failures: vec![],
/// };
/// assert_eq!(results.total, results.passed + results.failed + results.skipped);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestResults {
    /// Total tests run.
    pub total: u32,

    /// Tests passed.
    pub passed: u32,

    /// Tests failed.
    pub failed: u32,

    /// Tests skipped.
    pub skipped: u32,

    /// Individual failure details.
    pub failures: Vec<TestFailure>,
}

/// Details of a single test failure.
///
/// Contains information about a specific test that failed during
/// verification, including the error message and optional stack trace.
///
/// # Example
///
/// ```
/// use codirigent_core::TestFailure;
///
/// let failure = TestFailure {
///     name: "test_user_login".to_string(),
///     message: "Expected status 200, got 401".to_string(),
///     stack_trace: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestFailure {
    /// Test name/path.
    pub name: String,

    /// Error message.
    pub message: String,

    /// Stack trace if available.
    pub stack_trace: Option<String>,
}

/// Git worktree information.
///
/// Represents a git worktree, which allows multiple working directories
/// to be associated with a single repository. This enables parallel
/// development across multiple AI sessions without branch conflicts.
///
/// # Example
///
/// ```
/// use codirigent_core::Worktree;
/// use std::path::PathBuf;
///
/// let worktree = Worktree::new(
///     PathBuf::from("/repo/worktrees/feature"),
///     "feature-branch".to_string(),
///     false,
/// );
/// assert_eq!(worktree.branch, "feature-branch");
/// assert!(!worktree.is_main);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Worktree {
    /// Absolute path to the worktree directory.
    pub path: PathBuf,
    /// Branch name associated with this worktree.
    pub branch: String,
    /// Head commit SHA (short form, typically 8 characters).
    pub head_sha: Option<String>,
    /// Whether this is the main worktree (the original repository).
    pub is_main: bool,
    /// Session bound to this worktree, if any.
    pub bound_session: Option<SessionId>,
    /// When the worktree was created or first detected.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Worktree {
    /// Create a new worktree instance.
    ///
    /// # Arguments
    ///
    /// * `path` - Absolute path to the worktree directory
    /// * `branch` - Branch name associated with this worktree
    /// * `is_main` - Whether this is the main worktree
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::Worktree;
    /// use std::path::PathBuf;
    ///
    /// let wt = Worktree::new(
    ///     PathBuf::from("/repo"),
    ///     "main".to_string(),
    ///     true,
    /// );
    /// assert!(wt.is_main);
    /// ```
    pub fn new(path: PathBuf, branch: String, is_main: bool) -> Self {
        Self {
            path,
            branch,
            head_sha: None,
            is_main,
            bound_session: None,
            created_at: chrono::Utc::now(),
        }
    }

    /// Set the head SHA for this worktree.
    ///
    /// # Arguments
    ///
    /// * `sha` - The commit SHA (typically truncated to 8 characters)
    pub fn with_head_sha(mut self, sha: String) -> Self {
        self.head_sha = Some(sha);
        self
    }
}

/// Options for creating a new worktree.
///
/// Specifies the branch name and optional configuration for
/// creating a new git worktree.
///
/// # Example
///
/// ```
/// use codirigent_core::WorktreeCreateOptions;
///
/// let options = WorktreeCreateOptions::new("feature-branch".to_string())
///     .with_base_branch("main".to_string());
/// assert_eq!(options.branch, "feature-branch");
/// assert_eq!(options.base_branch, Some("main".to_string()));
/// ```
#[derive(Debug, Clone)]
pub struct WorktreeCreateOptions {
    /// Branch name to checkout (creates if doesn't exist).
    pub branch: String,
    /// Base branch to create from (if creating new branch).
    pub base_branch: Option<String>,
    /// Custom path for the worktree (defaults to ./worktrees/<branch>).
    pub path: Option<PathBuf>,
}

impl WorktreeCreateOptions {
    /// Create new worktree options with the given branch name.
    ///
    /// # Arguments
    ///
    /// * `branch` - The branch name to checkout or create
    pub fn new(branch: String) -> Self {
        Self {
            branch,
            base_branch: None,
            path: None,
        }
    }

    /// Set the base branch to create from.
    ///
    /// If the branch doesn't exist, it will be created from this base.
    ///
    /// # Arguments
    ///
    /// * `base` - The base branch name (e.g., "main", "develop")
    pub fn with_base_branch(mut self, base: String) -> Self {
        self.base_branch = Some(base);
        self
    }

    /// Set a custom path for the worktree.
    ///
    /// By default, worktrees are created in ./worktrees/<branch>.
    ///
    /// # Arguments
    ///
    /// * `path` - Custom path for the worktree directory
    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SessionId tests
    #[test]
    fn test_session_id_display() {
        let id = SessionId(42);
        assert_eq!(format!("{}", id), "session-42");
    }

    #[test]
    fn test_session_id_equality() {
        assert_eq!(SessionId(1), SessionId(1));
        assert_ne!(SessionId(1), SessionId(2));
    }

    #[test]
    fn test_session_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SessionId(1));
        assert!(set.contains(&SessionId(1)));
        assert!(!set.contains(&SessionId(2)));
    }

    #[test]
    fn test_session_id_serialization() {
        let id = SessionId(42);
        let json = serde_json::to_string(&id).unwrap();
        let parsed: SessionId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_session_id_clone() {
        let id = SessionId(42);
        let cloned = id;
        assert_eq!(id, cloned);
    }

    // TaskId tests
    #[test]
    fn test_task_id_display() {
        let id = TaskId("task-001".to_string());
        assert_eq!(format!("{}", id), "task-001");
    }

    #[test]
    fn test_task_id_equality() {
        let id1 = TaskId("task-001".to_string());
        let id2 = TaskId("task-001".to_string());
        let id3 = TaskId("task-002".to_string());
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_task_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TaskId("task-001".to_string()));
        assert!(set.contains(&TaskId("task-001".to_string())));
        assert!(!set.contains(&TaskId("task-002".to_string())));
    }

    #[test]
    fn test_task_id_serialization() {
        let id = TaskId("task-001".to_string());
        let json = serde_json::to_string(&id).unwrap();
        let parsed: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    // SessionStatus tests
    #[test]
    fn test_session_status_default() {
        assert_eq!(SessionStatus::default(), SessionStatus::Idle);
    }

    #[test]
    fn test_session_status_serialization() {
        let status = SessionStatus::NeedsAttention;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"NeedsAttention\"");

        let parsed: SessionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, parsed);
    }

    #[test]
    fn test_session_status_all_variants() {
        let variants = [
            SessionStatus::Idle,
            SessionStatus::Working,
            SessionStatus::NeedsAttention,
            SessionStatus::Error,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: SessionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // TaskPriority tests
    #[test]
    fn test_task_priority_default() {
        assert_eq!(TaskPriority::default(), TaskPriority::Medium);
    }

    #[test]
    fn test_task_priority_serialization() {
        let priority = TaskPriority::Critical;
        let json = serde_json::to_string(&priority).unwrap();
        assert_eq!(json, "\"Critical\"");

        let parsed: TaskPriority = serde_json::from_str(&json).unwrap();
        assert_eq!(priority, parsed);
    }

    #[test]
    fn test_task_priority_all_variants() {
        let variants = [
            TaskPriority::Critical,
            TaskPriority::High,
            TaskPriority::Medium,
            TaskPriority::Low,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: TaskPriority = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // TaskStatus tests
    #[test]
    fn test_task_status_default() {
        assert_eq!(TaskStatus::default(), TaskStatus::Queued);
    }

    #[test]
    fn test_task_status_serialization() {
        let status = TaskStatus::Verifying;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Verifying\"");

        let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, parsed);
    }

    #[test]
    fn test_task_status_all_variants() {
        let variants = [
            TaskStatus::Queued,
            TaskStatus::Assigned,
            TaskStatus::Working,
            TaskStatus::Verifying,
            TaskStatus::Review,
            TaskStatus::Done,
            TaskStatus::Blocked,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // VerificationConfig tests
    #[test]
    fn test_verification_config_default() {
        let config = VerificationConfig::default();
        assert!(config.command.is_empty());
        assert!(config.requires_human_review);
        assert_eq!(config.timeout, Duration::from_secs(300));
        assert!(config.working_dir.is_none());
        assert!(config.success_patterns.is_empty());
        assert!(config.failure_patterns.is_empty());
    }

    #[test]
    fn test_verification_config_serialization() {
        let config = VerificationConfig {
            command: "npm test".to_string(),
            working_dir: None,
            timeout: Duration::from_secs(60),
            requires_human_review: false,
            success_patterns: vec!["PASS".to_string()],
            failure_patterns: vec!["FAIL".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: VerificationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "npm test");
        assert_eq!(parsed.timeout, Duration::from_secs(60));
        assert!(!parsed.requires_human_review);
        assert_eq!(parsed.success_patterns, vec!["PASS".to_string()]);
        assert_eq!(parsed.failure_patterns, vec!["FAIL".to_string()]);
    }

    #[test]
    fn test_verification_config_with_working_dir() {
        let config = VerificationConfig {
            command: "cargo test".to_string(),
            working_dir: Some(PathBuf::from("/project")),
            timeout: Duration::from_secs(120),
            requires_human_review: true,
            success_patterns: Vec::new(),
            failure_patterns: Vec::new(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: VerificationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.working_dir, Some(PathBuf::from("/project")));
    }

    #[test]
    fn test_verification_config_humantime_serialization() {
        let config = VerificationConfig {
            command: "test".to_string(),
            working_dir: None,
            timeout: Duration::from_secs(300),
            requires_human_review: true,
            success_patterns: Vec::new(),
            failure_patterns: Vec::new(),
        };
        let json = serde_json::to_string(&config).unwrap();
        // humantime-serde serializes Duration as human-readable strings like "5m"
        assert!(json.contains("5m") || json.contains("300"));
    }

    // RetryConfig tests
    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_count, 0);
        assert_eq!(config.retry_delay, Duration::from_secs(0));
    }

    #[test]
    fn test_retry_config_serialization() {
        let config = RetryConfig {
            max_retries: 5,
            retry_count: 2,
            retry_delay: Duration::from_secs(30),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: RetryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_retries, 5);
        assert_eq!(parsed.retry_count, 2);
        assert_eq!(parsed.retry_delay, Duration::from_secs(30));
    }

    #[test]
    fn test_retry_config_equality() {
        let config1 = RetryConfig::default();
        let config2 = RetryConfig::default();
        let config3 = RetryConfig {
            max_retries: 5,
            ..Default::default()
        };
        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    // QueueState tests
    #[test]
    fn test_queue_state_default() {
        let state = QueueState::default();
        assert!(state.order.is_empty());
        assert!(state.blocked.is_empty());
        assert!(state.updated_at.is_none());
    }

    #[test]
    fn test_queue_state_serialization() {
        let state = QueueState {
            order: vec![
                TaskId("task-001".to_string()),
                TaskId("task-002".to_string()),
            ],
            blocked: {
                let mut m = HashMap::new();
                m.insert(
                    TaskId("task-003".to_string()),
                    vec![TaskId("task-001".to_string())],
                );
                m
            },
            updated_at: Some(chrono::Utc::now()),
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        let parsed: QueueState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.order.len(), 2);
        assert!(parsed.blocked.contains_key(&TaskId("task-003".to_string())));
        assert!(parsed.updated_at.is_some());
    }

    #[test]
    fn test_queue_state_equality() {
        let state1 = QueueState::default();
        let state2 = QueueState::default();
        let mut state3 = QueueState::default();
        state3.order.push(TaskId("task-001".to_string()));
        assert_eq!(state1, state2);
        assert_ne!(state1, state3);
    }

    // VerificationResult tests
    #[test]
    fn test_verification_result_success() {
        let result = VerificationResult {
            success: true,
            exit_code: Some(0),
            stdout: "All tests passed".to_string(),
            stderr: "".to_string(),
            test_results: None,
            duration: Duration::from_secs(5),
            run_at: chrono::Utc::now(),
        };
        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_verification_result_failure() {
        let result = VerificationResult {
            success: false,
            exit_code: Some(1),
            stdout: "21 passed, 2 failed".to_string(),
            stderr: "".to_string(),
            test_results: Some(TestResults {
                total: 23,
                passed: 21,
                failed: 2,
                skipped: 0,
                failures: vec![TestFailure {
                    name: "auth.test > should reject".to_string(),
                    message: "Expected 401, got 200".to_string(),
                    stack_trace: None,
                }],
            }),
            duration: Duration::from_secs(15),
            run_at: chrono::Utc::now(),
        };

        assert!(!result.success);
        let test_results = result.test_results.unwrap();
        assert_eq!(test_results.failed, 2);
        assert_eq!(test_results.failures.len(), 1);
    }

    #[test]
    fn test_verification_result_serialization() {
        let result = VerificationResult {
            success: false,
            exit_code: Some(1),
            stdout: "21 passed, 2 failed".to_string(),
            stderr: "warning".to_string(),
            test_results: Some(TestResults {
                total: 23,
                passed: 21,
                failed: 2,
                skipped: 0,
                failures: vec![TestFailure {
                    name: "auth.test > should reject".to_string(),
                    message: "Expected 401, got 200".to_string(),
                    stack_trace: Some("at line 42".to_string()),
                }],
            }),
            duration: Duration::from_secs(15),
            run_at: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: VerificationResult = serde_json::from_str(&json).unwrap();
        assert!(!parsed.success);
        assert_eq!(parsed.test_results.unwrap().failed, 2);
    }

    // TestResults tests
    #[test]
    fn test_test_results_creation() {
        let results = TestResults {
            total: 10,
            passed: 8,
            failed: 1,
            skipped: 1,
            failures: vec![],
        };
        assert_eq!(
            results.total,
            results.passed + results.failed + results.skipped
        );
    }

    #[test]
    fn test_test_results_serialization() {
        let results = TestResults {
            total: 10,
            passed: 10,
            failed: 0,
            skipped: 0,
            failures: vec![],
        };
        let json = serde_json::to_string(&results).unwrap();
        let parsed: TestResults = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total, 10);
        assert_eq!(parsed.passed, 10);
    }

    // TestFailure tests
    #[test]
    fn test_test_failure_creation() {
        let failure = TestFailure {
            name: "test_login".to_string(),
            message: "assertion failed".to_string(),
            stack_trace: Some("at main.rs:42".to_string()),
        };
        assert_eq!(failure.name, "test_login");
        assert!(failure.stack_trace.is_some());
    }

    #[test]
    fn test_test_failure_serialization() {
        let failure = TestFailure {
            name: "test_login".to_string(),
            message: "assertion failed".to_string(),
            stack_trace: None,
        };
        let json = serde_json::to_string(&failure).unwrap();
        let parsed: TestFailure = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test_login");
        assert!(parsed.stack_trace.is_none());
    }

    // GridPosition tests
    #[test]
    fn test_grid_position_creation() {
        let pos = GridPosition { row: 1, col: 2 };
        assert_eq!(pos.row, 1);
        assert_eq!(pos.col, 2);
    }

    #[test]
    fn test_grid_position_equality() {
        let pos1 = GridPosition { row: 0, col: 0 };
        let pos2 = GridPosition { row: 0, col: 0 };
        let pos3 = GridPosition { row: 1, col: 0 };
        assert_eq!(pos1, pos2);
        assert_ne!(pos1, pos3);
    }

    #[test]
    fn test_grid_position_serialization() {
        let pos = GridPosition { row: 1, col: 2 };
        let json = serde_json::to_string(&pos).unwrap();
        let parsed: GridPosition = serde_json::from_str(&json).unwrap();
        assert_eq!(pos, parsed);
    }

    // LayoutMode tests
    #[test]
    fn test_layout_mode_default() {
        let layout = LayoutMode::default();
        assert!(matches!(layout, LayoutMode::Grid { rows: 2, cols: 2 }));
    }

    #[test]
    fn test_layout_mode_grid_serialization() {
        let layout = LayoutMode::Grid { rows: 3, cols: 3 };
        let json = serde_json::to_string(&layout).unwrap();
        let parsed: LayoutMode = serde_json::from_str(&json).unwrap();
        assert_eq!(layout, parsed);
    }

    #[test]
    fn test_layout_mode_single() {
        let layout = LayoutMode::Single;
        let json = serde_json::to_string(&layout).unwrap();
        let parsed: LayoutMode = serde_json::from_str(&json).unwrap();
        assert_eq!(layout, parsed);
    }

    #[test]
    fn test_layout_mode_custom() {
        let positions = vec![
            (SessionId(1), GridPosition { row: 0, col: 0 }),
            (SessionId(2), GridPosition { row: 0, col: 1 }),
        ];
        let layout = LayoutMode::Custom {
            positions: positions.clone(),
        };
        let json = serde_json::to_string(&layout).unwrap();
        let parsed: LayoutMode = serde_json::from_str(&json).unwrap();
        assert_eq!(layout, parsed);
    }

    // Session tests
    #[test]
    fn test_session_new() {
        let session = Session::new(
            SessionId(1),
            "Test Session".to_string(),
            PathBuf::from("/tmp"),
        );
        assert_eq!(session.id, SessionId(1));
        assert_eq!(session.name, "Test Session");
        assert_eq!(session.status, SessionStatus::Idle);
        assert_eq!(session.working_directory, PathBuf::from("/tmp"));
        assert!(session.current_task.is_none());
        assert!(session.context_usage.is_none());
        assert!(session.group.is_none());
        assert!(session.color.is_none());
    }

    #[test]
    fn test_session_serialization() {
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        let json = serde_json::to_string_pretty(&session).unwrap();
        let parsed: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(session.id, parsed.id);
        assert_eq!(session.name, parsed.name);
        assert_eq!(session.status, parsed.status);
    }

    #[test]
    fn test_session_with_all_fields() {
        let mut session = Session::new(
            SessionId(1),
            "Full Session".to_string(),
            PathBuf::from("/home/user"),
        );
        session.status = SessionStatus::Working;
        session.current_task = Some(TaskId("task-001".to_string()));
        session.context_usage = Some(0.75);
        session.group = Some("backend".to_string());
        session.color = Some("#FF5733".to_string());

        let json = serde_json::to_string(&session).unwrap();
        let parsed: Session = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.status, SessionStatus::Working);
        assert_eq!(parsed.current_task, Some(TaskId("task-001".to_string())));
        assert_eq!(parsed.context_usage, Some(0.75));
        assert_eq!(parsed.group, Some("backend".to_string()));
        assert_eq!(parsed.color, Some("#FF5733".to_string()));
    }

    // Task tests
    #[test]
    fn test_task_new() {
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test Task".to_string(),
            "A test task description".to_string(),
        );
        assert_eq!(task.id, TaskId("task-001".to_string()));
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.description, "A test task description");
        assert_eq!(task.priority, TaskPriority::Medium);
        assert_eq!(task.status, TaskStatus::Queued);
        assert!(task.dependencies.is_empty());
        assert!(task.tags.is_empty());
        assert!(task.assigned_session.is_none());
        assert!(task.verification.is_none());
        assert!(task.can_retry());
        assert!(task.estimated_minutes.is_none());
        assert!(task.error_message.is_none());
        assert!(task.project_dir.is_none());
        assert!(task.plan_file.is_none());
    }

    #[test]
    fn test_task_serialization() {
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "Description".to_string(),
        );
        let json = serde_json::to_string_pretty(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(task.id, parsed.id);
        assert_eq!(task.title, parsed.title);
    }

    #[test]
    fn test_task_with_all_fields() {
        let mut task = Task::new(
            TaskId("task-001".to_string()),
            "Full Task".to_string(),
            "Full description".to_string(),
        );
        task.priority = TaskPriority::High;
        task.status = TaskStatus::Working;
        task.dependencies = vec![TaskId("task-000".to_string())];
        task.tags = vec!["backend".to_string(), "urgent".to_string()];
        task.assigned_session = Some(SessionId(1));
        task.assigned_at = Some(chrono::Utc::now());
        task.started_at = Some(chrono::Utc::now());
        task.estimated_minutes = Some(30);
        task.error_message = Some("Test error".to_string());

        let json = serde_json::to_string(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.priority, TaskPriority::High);
        assert_eq!(parsed.status, TaskStatus::Working);
        assert_eq!(parsed.dependencies.len(), 1);
        assert_eq!(parsed.tags.len(), 2);
        assert_eq!(parsed.assigned_session, Some(SessionId(1)));
        assert_eq!(parsed.estimated_minutes, Some(30));
        assert_eq!(parsed.error_message, Some("Test error".to_string()));
    }

    #[test]
    fn test_task_with_verification() {
        let mut task = Task::new(
            TaskId("task-002".to_string()),
            "Verified Task".to_string(),
            "Run with tests".to_string(),
        );
        task.verification = Some(VerificationConfig {
            command: "cargo test".to_string(),
            ..Default::default()
        });
        assert!(task.verification.is_some());

        let json = serde_json::to_string(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert!(parsed.verification.is_some());
        assert_eq!(parsed.verification.unwrap().command, "cargo test");
    }

    #[test]
    fn test_task_retry_logic() {
        let mut task = Task::new(
            TaskId("task-003".to_string()),
            "Retry Task".to_string(),
            "May fail".to_string(),
        );
        assert!(task.can_retry());
        assert_eq!(task.retry.retry_count, 0);

        task.increment_retry();
        assert_eq!(task.retry.retry_count, 1);
        assert!(task.can_retry());

        task.increment_retry();
        task.increment_retry();
        assert_eq!(task.retry.retry_count, 3);
        assert!(!task.can_retry());
    }

    #[test]
    fn test_task_project_dir_and_plan_file() {
        let mut task = Task::new(
            TaskId("task-001".to_string()),
            "Project Task".to_string(),
            "Description".to_string(),
        );
        task.project_dir = Some(PathBuf::from("/home/user/project"));
        task.plan_file = Some("plans/phase-1.md".to_string());

        let json = serde_json::to_string(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(
            parsed.project_dir,
            Some(PathBuf::from("/home/user/project"))
        );
        assert_eq!(parsed.plan_file, Some("plans/phase-1.md".to_string()));
    }

    #[test]
    fn test_task_backwards_compat_without_project_fields() {
        // Simulate old JSON without project_dir and plan_file fields
        let json = r#"{
            "id": "task-old",
            "title": "Old Task",
            "description": "No project fields",
            "priority": "Medium",
            "status": "Queued",
            "dependencies": [],
            "tags": [],
            "estimated_minutes": null,
            "assigned_session": null,
            "assigned_at": null,
            "verification": null,
            "retry": {"max_retries": 3, "retry_count": 0, "retry_delay": "0s"},
            "created_at": "2025-01-01T00:00:00Z",
            "started_at": null,
            "completed_at": null,
            "error_message": null
        }"#;

        let parsed: Task = serde_json::from_str(json).unwrap();
        assert!(parsed.project_dir.is_none());
        assert!(parsed.plan_file.is_none());
    }

    #[test]
    fn test_task_project_dir_skip_serializing_if_none() {
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Task".to_string(),
            "Desc".to_string(),
        );
        let json = serde_json::to_string(&task).unwrap();
        assert!(!json.contains("project_dir"));
        assert!(!json.contains("plan_file"));
    }

    #[test]
    fn test_task_dependencies_satisfied_empty() {
        let task = Task::new(
            TaskId("task-001".to_string()),
            "No deps".to_string(),
            "".to_string(),
        );
        assert!(task.dependencies_satisfied(&[]));
        assert!(task.dependencies_satisfied(&[TaskId("task-other".to_string())]));
    }

    #[test]
    fn test_task_dependencies_satisfied_with_deps() {
        let mut task = Task::new(
            TaskId("task-002".to_string()),
            "Has deps".to_string(),
            "".to_string(),
        );
        task.dependencies = vec![
            TaskId("task-001".to_string()),
            TaskId("task-000".to_string()),
        ];

        // Not satisfied
        assert!(!task.dependencies_satisfied(&[]));
        assert!(!task.dependencies_satisfied(&[TaskId("task-001".to_string())]));

        // Satisfied
        assert!(task.dependencies_satisfied(&[
            TaskId("task-001".to_string()),
            TaskId("task-000".to_string()),
        ]));

        // Satisfied with extra tasks
        assert!(task.dependencies_satisfied(&[
            TaskId("task-001".to_string()),
            TaskId("task-000".to_string()),
            TaskId("task-extra".to_string()),
        ]));
    }

    // AppState tests
    #[test]
    fn test_app_state_default() {
        let state = AppState::default();
        assert!(state.sessions.is_empty());
        assert!(matches!(
            state.layout,
            LayoutMode::Grid { rows: 2, cols: 2 }
        ));
        assert!(state.updated_at.is_none());
    }

    #[test]
    fn test_app_state_serialization() {
        let mut state = AppState::default();
        state.sessions.push(Session::new(
            SessionId(1),
            "Test".to_string(),
            PathBuf::from("/tmp"),
        ));
        state.updated_at = Some(chrono::Utc::now());

        let json = serde_json::to_string_pretty(&state).unwrap();
        let parsed: AppState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.sessions.len(), parsed.sessions.len());
        assert_eq!(parsed.sessions[0].id, SessionId(1));
    }

    #[test]
    fn test_app_state_with_multiple_sessions() {
        let mut state = AppState::default();
        state.sessions.push(Session::new(
            SessionId(1),
            "Session 1".to_string(),
            PathBuf::from("/tmp/1"),
        ));
        state.sessions.push(Session::new(
            SessionId(2),
            "Session 2".to_string(),
            PathBuf::from("/tmp/2"),
        ));
        state.layout = LayoutMode::Grid { rows: 1, cols: 2 };

        let json = serde_json::to_string(&state).unwrap();
        let parsed: AppState = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.sessions.len(), 2);
        assert!(matches!(
            parsed.layout,
            LayoutMode::Grid { rows: 1, cols: 2 }
        ));
    }

    // GitRepoInfo tests
    #[test]
    fn test_git_repo_info_creation() {
        let info = GitRepoInfo {
            repo_root: PathBuf::from("/home/user/project"),
            branch: "main".to_string(),
            dirty_count: 3,
            has_staged: true,
            head_sha: Some("abc12345".to_string()),
            unstaged_files: vec![],
            staged_files: vec![],
        };
        assert_eq!(info.branch, "main");
        assert_eq!(info.dirty_count, 3);
        assert!(info.has_staged);
        assert_eq!(info.head_sha, Some("abc12345".to_string()));
    }

    #[test]
    fn test_git_repo_info_serialization() {
        let info = GitRepoInfo {
            repo_root: PathBuf::from("/repo"),
            branch: "feature/test".to_string(),
            dirty_count: 0,
            has_staged: false,
            head_sha: None,
            unstaged_files: vec![],
            staged_files: vec![],
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: GitRepoInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, parsed);
    }

    #[test]
    fn test_git_repo_info_equality() {
        let info1 = GitRepoInfo {
            repo_root: PathBuf::from("/repo"),
            branch: "main".to_string(),
            dirty_count: 0,
            has_staged: false,
            head_sha: None,
            unstaged_files: vec![],
            staged_files: vec![],
        };
        let info2 = info1.clone();
        assert_eq!(info1, info2);

        let info3 = GitRepoInfo {
            repo_root: PathBuf::from("/repo"),
            branch: "develop".to_string(),
            dirty_count: 0,
            has_staged: false,
            head_sha: None,
            unstaged_files: vec![],
            staged_files: vec![],
        };
        assert_ne!(info1, info3);
    }

    #[test]
    fn test_session_with_git_info() {
        let mut session = Session::new(
            SessionId(1),
            "Git Session".to_string(),
            PathBuf::from("/project"),
        );
        assert!(session.git_info.is_none());

        session.git_info = Some(GitRepoInfo {
            repo_root: PathBuf::from("/project"),
            branch: "main".to_string(),
            dirty_count: 2,
            has_staged: true,
            head_sha: Some("deadbeef".to_string()),
            unstaged_files: vec![],
            staged_files: vec![],
        });

        let json = serde_json::to_string(&session).unwrap();
        let parsed: Session = serde_json::from_str(&json).unwrap();
        assert!(parsed.git_info.is_some());
        let gi = parsed.git_info.unwrap();
        assert_eq!(gi.branch, "main");
        assert_eq!(gi.dirty_count, 2);
    }

    // Worktree tests
    #[test]
    fn test_worktree_new() {
        let wt = Worktree::new(
            PathBuf::from("/repo/worktrees/feature"),
            "feature-branch".to_string(),
            false,
        );
        assert_eq!(wt.path, PathBuf::from("/repo/worktrees/feature"));
        assert_eq!(wt.branch, "feature-branch");
        assert!(!wt.is_main);
        assert!(wt.head_sha.is_none());
        assert!(wt.bound_session.is_none());
    }

    #[test]
    fn test_worktree_main() {
        let wt = Worktree::new(PathBuf::from("/repo"), "main".to_string(), true);
        assert!(wt.is_main);
        assert_eq!(wt.branch, "main");
    }

    #[test]
    fn test_worktree_with_head_sha() {
        let wt = Worktree::new(PathBuf::from("/repo"), "main".to_string(), true)
            .with_head_sha("abc12345".to_string());
        assert_eq!(wt.head_sha, Some("abc12345".to_string()));
    }

    #[test]
    fn test_worktree_serialization() {
        let wt = Worktree::new(PathBuf::from("/repo"), "main".to_string(), true)
            .with_head_sha("abc12345".to_string());
        let json = serde_json::to_string(&wt).unwrap();
        let parsed: Worktree = serde_json::from_str(&json).unwrap();
        assert_eq!(wt.branch, parsed.branch);
        assert_eq!(wt.head_sha, parsed.head_sha);
        assert_eq!(wt.is_main, parsed.is_main);
    }

    #[test]
    fn test_worktree_equality() {
        let wt1 = Worktree::new(PathBuf::from("/repo"), "main".to_string(), true);
        let wt2 = Worktree::new(PathBuf::from("/repo"), "main".to_string(), true);
        // Note: created_at will differ, but we compare specific fields
        assert_eq!(wt1.branch, wt2.branch);
        assert_eq!(wt1.path, wt2.path);
        assert_eq!(wt1.is_main, wt2.is_main);
    }

    #[test]
    fn test_worktree_with_bound_session() {
        let mut wt = Worktree::new(PathBuf::from("/repo"), "feature".to_string(), false);
        wt.bound_session = Some(SessionId(42));
        assert_eq!(wt.bound_session, Some(SessionId(42)));

        let json = serde_json::to_string(&wt).unwrap();
        let parsed: Worktree = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.bound_session, Some(SessionId(42)));
    }

    #[test]
    fn test_worktree_clone() {
        let wt = Worktree::new(PathBuf::from("/repo"), "main".to_string(), true);
        let cloned = wt.clone();
        assert_eq!(wt.branch, cloned.branch);
        assert_eq!(wt.path, cloned.path);
    }

    #[test]
    fn test_worktree_debug() {
        let wt = Worktree::new(PathBuf::from("/repo"), "main".to_string(), true);
        let debug_str = format!("{:?}", wt);
        assert!(debug_str.contains("Worktree"));
        assert!(debug_str.contains("main"));
    }

    // WorktreeCreateOptions tests
    #[test]
    fn test_worktree_create_options_new() {
        let opts = WorktreeCreateOptions::new("feature".to_string());
        assert_eq!(opts.branch, "feature");
        assert!(opts.base_branch.is_none());
        assert!(opts.path.is_none());
    }

    #[test]
    fn test_worktree_create_options_with_base_branch() {
        let opts =
            WorktreeCreateOptions::new("feature".to_string()).with_base_branch("main".to_string());
        assert_eq!(opts.branch, "feature");
        assert_eq!(opts.base_branch, Some("main".to_string()));
    }

    #[test]
    fn test_worktree_create_options_with_path() {
        let opts = WorktreeCreateOptions::new("feature".to_string())
            .with_path(PathBuf::from("/custom/path"));
        assert_eq!(opts.path, Some(PathBuf::from("/custom/path")));
    }

    #[test]
    fn test_worktree_create_options_all_fields() {
        let opts = WorktreeCreateOptions::new("feature".to_string())
            .with_base_branch("develop".to_string())
            .with_path(PathBuf::from("/worktrees/feature"));
        assert_eq!(opts.branch, "feature");
        assert_eq!(opts.base_branch, Some("develop".to_string()));
        assert_eq!(opts.path, Some(PathBuf::from("/worktrees/feature")));
    }

    #[test]
    fn test_worktree_create_options_clone() {
        let opts =
            WorktreeCreateOptions::new("feature".to_string()).with_base_branch("main".to_string());
        let cloned = opts.clone();
        assert_eq!(opts.branch, cloned.branch);
        assert_eq!(opts.base_branch, cloned.base_branch);
    }

    #[test]
    fn test_worktree_create_options_debug() {
        let opts = WorktreeCreateOptions::new("feature".to_string());
        let debug_str = format!("{:?}", opts);
        assert!(debug_str.contains("WorktreeCreateOptions"));
        assert!(debug_str.contains("feature"));
    }

    // ContextThresholdState tests
    #[test]
    fn test_context_threshold_state_default() {
        assert_eq!(
            ContextThresholdState::default(),
            ContextThresholdState::Normal
        );
    }

    #[test]
    fn test_context_threshold_state_equality() {
        assert_eq!(ContextThresholdState::Normal, ContextThresholdState::Normal);
        assert_ne!(
            ContextThresholdState::Normal,
            ContextThresholdState::Warning
        );
        assert_ne!(
            ContextThresholdState::Warning,
            ContextThresholdState::Critical
        );
    }

    #[test]
    fn test_context_threshold_state_serialization() {
        let states = [
            ContextThresholdState::Normal,
            ContextThresholdState::Warning,
            ContextThresholdState::Critical,
        ];
        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let parsed: ContextThresholdState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, parsed);
        }
    }

    #[test]
    fn test_context_threshold_state_clone_copy() {
        let state = ContextThresholdState::Warning;
        let cloned = state;
        assert_eq!(state, cloned);
    }

    #[test]
    fn test_context_threshold_state_debug() {
        let state = ContextThresholdState::Critical;
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("Critical"));
    }

    // SlotId tests
    #[test]
    fn test_slot_id_display() {
        let id = SlotId(42);
        assert_eq!(format!("{}", id), "slot-42");
    }

    #[test]
    fn test_slot_id_equality() {
        assert_eq!(SlotId(1), SlotId(1));
        assert_ne!(SlotId(1), SlotId(2));
    }

    #[test]
    fn test_slot_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SlotId(1));
        assert!(set.contains(&SlotId(1)));
        assert!(!set.contains(&SlotId(2)));
    }

    #[test]
    fn test_slot_id_serialization() {
        let id = SlotId(42);
        let json = serde_json::to_string(&id).unwrap();
        let parsed: SlotId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    // SplitDirection tests
    #[test]
    fn test_split_direction_serialization() {
        let directions = [SplitDirection::Horizontal, SplitDirection::Vertical];
        for dir in directions {
            let json = serde_json::to_string(&dir).unwrap();
            let parsed: SplitDirection = serde_json::from_str(&json).unwrap();
            assert_eq!(dir, parsed);
        }
    }

    // LayoutNode tests
    #[test]
    fn test_layout_node_leaf() {
        let node = LayoutNode::Leaf { slot: SlotId(0) };
        assert_eq!(node.leaf_count(), 1);
        assert_eq!(node.slots_in_order(), vec![SlotId(0)]);
    }

    #[test]
    fn test_layout_node_split() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf { slot: SlotId(0) }),
            second: Box::new(LayoutNode::Leaf { slot: SlotId(1) }),
        };
        assert_eq!(node.leaf_count(), 2);
        assert_eq!(node.slots_in_order(), vec![SlotId(0), SlotId(1)]);
    }

    #[test]
    fn test_layout_node_from_grid_1x1() {
        let node = LayoutNode::from_grid(1, 1);
        assert_eq!(node.leaf_count(), 1);
        assert!(matches!(node, LayoutNode::Leaf { slot: SlotId(0) }));
    }

    #[test]
    fn test_layout_node_from_grid_2x2() {
        let node = LayoutNode::from_grid(2, 2);
        assert_eq!(node.leaf_count(), 4);
        let slots = node.slots_in_order();
        assert_eq!(slots, vec![SlotId(0), SlotId(1), SlotId(2), SlotId(3)]);
    }

    #[test]
    fn test_layout_node_from_grid_1x4() {
        let node = LayoutNode::from_grid(1, 4);
        assert_eq!(node.leaf_count(), 4);
        let slots = node.slots_in_order();
        assert_eq!(slots, vec![SlotId(0), SlotId(1), SlotId(2), SlotId(3)]);
    }

    #[test]
    fn test_layout_node_from_grid_4x1() {
        let node = LayoutNode::from_grid(4, 1);
        assert_eq!(node.leaf_count(), 4);
        let slots = node.slots_in_order();
        assert_eq!(slots, vec![SlotId(0), SlotId(1), SlotId(2), SlotId(3)]);
    }

    #[test]
    fn test_layout_node_from_grid_2x3() {
        let node = LayoutNode::from_grid(2, 3);
        assert_eq!(node.leaf_count(), 6);
        let slots = node.slots_in_order();
        assert_eq!(
            slots,
            vec![SlotId(0), SlotId(1), SlotId(2), SlotId(3), SlotId(4), SlotId(5)]
        );
    }

    #[test]
    fn test_layout_node_split_slot() {
        let node = LayoutNode::Leaf { slot: SlotId(0) };
        let new_tree = node
            .split_slot(SlotId(0), SplitDirection::Horizontal, 0.5, SlotId(1))
            .unwrap();
        assert_eq!(new_tree.leaf_count(), 2);
        assert_eq!(new_tree.slots_in_order(), vec![SlotId(0), SlotId(1)]);
    }

    #[test]
    fn test_layout_node_split_slot_nested() {
        let tree = LayoutNode::from_grid(2, 2);
        // Split slot 2 vertically
        let new_tree = tree
            .split_slot(SlotId(2), SplitDirection::Vertical, 0.5, SlotId(4))
            .unwrap();
        assert_eq!(new_tree.leaf_count(), 5);
        assert!(new_tree.contains_slot(SlotId(4)));
    }

    #[test]
    fn test_layout_node_split_slot_not_found() {
        let node = LayoutNode::Leaf { slot: SlotId(0) };
        let result = node.split_slot(SlotId(99), SplitDirection::Horizontal, 0.5, SlotId(1));
        assert!(result.is_none());
    }

    #[test]
    fn test_layout_node_close_slot() {
        let tree = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf { slot: SlotId(0) }),
            second: Box::new(LayoutNode::Leaf { slot: SlotId(1) }),
        };
        let after = tree.close_slot(SlotId(0)).unwrap();
        assert!(matches!(after, LayoutNode::Leaf { slot: SlotId(1) }));
    }

    #[test]
    fn test_layout_node_close_slot_promotes_sibling() {
        let tree = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf { slot: SlotId(0) }),
            second: Box::new(LayoutNode::Leaf { slot: SlotId(1) }),
        };
        let after = tree.close_slot(SlotId(1)).unwrap();
        assert!(matches!(after, LayoutNode::Leaf { slot: SlotId(0) }));
    }

    #[test]
    fn test_layout_node_close_root_leaf_returns_none() {
        let node = LayoutNode::Leaf { slot: SlotId(0) };
        assert!(node.close_slot(SlotId(0)).is_none());
    }

    #[test]
    fn test_layout_node_close_slot_not_found() {
        let tree = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf { slot: SlotId(0) }),
            second: Box::new(LayoutNode::Leaf { slot: SlotId(1) }),
        };
        assert!(tree.close_slot(SlotId(99)).is_none());
    }

    #[test]
    fn test_layout_node_close_slot_nested() {
        // Build a tree: V(H(0,1), 2)
        let tree = LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            first: Box::new(LayoutNode::Split {
                direction: SplitDirection::Horizontal,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf { slot: SlotId(0) }),
                second: Box::new(LayoutNode::Leaf { slot: SlotId(1) }),
            }),
            second: Box::new(LayoutNode::Leaf { slot: SlotId(2) }),
        };
        // Close slot 0, should promote slot 1
        let after = tree.close_slot(SlotId(0)).unwrap();
        assert_eq!(after.leaf_count(), 2);
        assert_eq!(after.slots_in_order(), vec![SlotId(1), SlotId(2)]);
    }

    #[test]
    fn test_layout_node_set_ratio_for_slot() {
        let tree = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf { slot: SlotId(0) }),
            second: Box::new(LayoutNode::Leaf { slot: SlotId(1) }),
        };
        let new_tree = tree.set_ratio_for_slot(SlotId(0), 0.3).unwrap();
        match &new_tree {
            LayoutNode::Split { ratio, .. } => {
                assert!((ratio - 0.3).abs() < 0.001);
            }
            _ => panic!("Expected split"),
        }
    }

    #[test]
    fn test_layout_node_set_ratio_clamps() {
        let tree = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf { slot: SlotId(0) }),
            second: Box::new(LayoutNode::Leaf { slot: SlotId(1) }),
        };
        // Ratio should be clamped to 0.1..0.9
        let new_tree = tree.set_ratio_for_slot(SlotId(0), 0.0).unwrap();
        match &new_tree {
            LayoutNode::Split { ratio, .. } => {
                assert!((ratio - 0.1).abs() < 0.001);
            }
            _ => panic!("Expected split"),
        }
        let new_tree = tree.set_ratio_for_slot(SlotId(0), 1.0).unwrap();
        match &new_tree {
            LayoutNode::Split { ratio, .. } => {
                assert!((ratio - 0.9).abs() < 0.001);
            }
            _ => panic!("Expected split"),
        }
    }

    #[test]
    fn test_layout_node_contains_slot() {
        let tree = LayoutNode::from_grid(2, 2);
        assert!(tree.contains_slot(SlotId(0)));
        assert!(tree.contains_slot(SlotId(3)));
        assert!(!tree.contains_slot(SlotId(4)));
    }

    #[test]
    fn test_layout_node_serialization() {
        let tree = LayoutNode::from_grid(2, 2);
        let json = serde_json::to_string(&tree).unwrap();
        let parsed: LayoutNode = serde_json::from_str(&json).unwrap();
        assert_eq!(tree, parsed);
    }

    #[test]
    fn test_layout_mode_split_tree_serialization() {
        let tree = LayoutNode::from_grid(2, 2);
        let mode = LayoutMode::SplitTree { root: tree.clone() };
        let json = serde_json::to_string(&mode).unwrap();
        let parsed: LayoutMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, parsed);
    }

    #[test]
    fn test_layout_mode_backward_compat_grid() {
        // Old JSON for Grid should still deserialize
        let json = r#"{"Grid":{"rows":2,"cols":2}}"#;
        let parsed: LayoutMode = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, LayoutMode::Grid { rows: 2, cols: 2 }));
    }

    #[test]
    fn test_layout_mode_backward_compat_single() {
        let json = r#""Single""#;
        let parsed: LayoutMode = serde_json::from_str(json).unwrap();
        assert!(matches!(parsed, LayoutMode::Single));
    }

    #[test]
    fn test_layout_node_from_grid_3x3() {
        let node = LayoutNode::from_grid(3, 3);
        assert_eq!(node.leaf_count(), 9);
        let slots = node.slots_in_order();
        assert_eq!(slots.len(), 9);
        // Slots should be numbered 0-8
        for i in 0..9 {
            assert_eq!(slots[i], SlotId(i as u32));
        }
    }
}
