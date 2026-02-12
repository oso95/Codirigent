//! Task assignment management and routing.
//!
//! This module provides the [`AssignmentManager`] system for routing tasks to sessions
//! when they become idle. It supports both automatic and manual assignment modes,
//! with optional confirmation before assignment.
//!
//! ## Features
//!
//! - Automatic task assignment when sessions become idle
//! - Optional confirmation before assignment
//! - Customizable prompt templates
//! - Pending assignment tracking with expiration
//!
//! ## Example
//!
//! ```
//! use codirigent_core::{
//!     AssignmentManager, AssignmentConfig, AssignmentAction,
//!     Task, TaskId, Session, SessionId, TaskQueue, SchedulerConfig,
//!     DefaultEventBus,
//! };
//! use codirigent_core::traits::EventBus;
//! use std::sync::Arc;
//! use std::path::PathBuf;
//!
//! // Create an assignment manager with default configuration
//! let event_bus = Arc::new(DefaultEventBus::new(16));
//! let config = AssignmentConfig::default();
//! let mut manager = AssignmentManager::new(config, event_bus.clone());
//!
//! // Create a task queue
//! let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
//!
//! // Add a task
//! let task = Task::new(
//!     TaskId::from("task-001"),
//!     "Implement feature".to_string(),
//!     "Add new feature X".to_string(),
//! );
//! queue.enqueue(task).unwrap();
//!
//! // When a session becomes idle, check for assignment
//! let session = Session::new(SessionId(1), "Session 1".to_string(), PathBuf::from("/tmp"));
//! let action = manager.on_session_idle(&session, &mut queue, &[]);
//!
//! match action {
//!     Some(AssignmentAction::AssignNow { task_id, prompt, .. }) => {
//!         println!("Assign task {} with prompt:\n{}", task_id, prompt);
//!     }
//!     Some(AssignmentAction::AwaitConfirmation { task_id, .. }) => {
//!         println!("Waiting for confirmation to assign task {}", task_id);
//!     }
//!     Some(AssignmentAction::NoTask) => {
//!         println!("No tasks available");
//!     }
//!     None => {
//!         println!("Session not available for assignment");
//!     }
//! }
//! ```

use crate::events::CodirigentEvent;
use crate::scheduler::TaskQueue;
use crate::traits::EventBus;
use crate::types::*;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Default prompt template for task assignment.
///
/// This template includes placeholders that will be replaced with task details:
/// - `{title}` - The task title
/// - `{id}` - The task ID
/// - `{priority}` - The task priority level
/// - `{description}` - The task description
/// - `{verification_command}` - The verification command (or "(auto-detect)" if none)
///
/// # Example
///
/// ```
/// use codirigent_core::DEFAULT_PROMPT_TEMPLATE;
///
/// assert!(DEFAULT_PROMPT_TEMPLATE.contains("{title}"));
/// assert!(DEFAULT_PROMPT_TEMPLATE.contains("{description}"));
/// ```
pub const DEFAULT_PROMPT_TEMPLATE: &str = r#"## Task: {title}

**Task ID:** {id}
**Priority:** {priority}

### Description

{description}

### Plan File

{plan_file}

### Verification

When complete, verification will run: `{verification_command}`

---

Please complete this task. When finished, indicate completion so verification can run.
"#;

/// Configuration for task assignment.
///
/// Controls how tasks are assigned to sessions, including whether
/// assignments are automatic or require confirmation.
///
/// # Example
///
/// ```
/// use codirigent_core::AssignmentConfig;
///
/// let config = AssignmentConfig::default();
/// assert!(config.auto_assign);
/// assert!(!config.confirm_before_assign);
/// assert_eq!(config.idle_threshold_seconds, 5);
///
/// // Custom configuration
/// let config = AssignmentConfig {
///     auto_assign: true,
///     confirm_before_assign: true,
///     idle_threshold_seconds: 10,
///     prompt_template: "Custom: {title}".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssignmentConfig {
    /// Whether to auto-assign tasks when sessions become idle.
    pub auto_assign: bool,

    /// Whether to confirm before auto-assigning.
    pub confirm_before_assign: bool,

    /// Seconds of idle time before considering a session available.
    pub idle_threshold_seconds: u32,

    /// Template for task prompts.
    pub prompt_template: String,
}

impl Default for AssignmentConfig {
    fn default() -> Self {
        Self {
            auto_assign: true,
            confirm_before_assign: false,
            idle_threshold_seconds: 5,
            prompt_template: DEFAULT_PROMPT_TEMPLATE.to_string(),
        }
    }
}

/// Pending assignment waiting for confirmation.
///
/// When `confirm_before_assign` is enabled, assignments are held in a pending
/// state until the user confirms or rejects them.
///
/// # Example
///
/// ```
/// use codirigent_core::{PendingAssignment, TaskId, SessionId};
///
/// let pending = PendingAssignment {
///     task_id: TaskId::from("task-001"),
///     session_id: SessionId(1),
///     prompt: "Task prompt here".to_string(),
///     proposed_at: chrono::Utc::now(),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct PendingAssignment {
    /// Task to assign.
    pub task_id: TaskId,
    /// Session to assign to.
    pub session_id: SessionId,
    /// Generated prompt.
    pub prompt: String,
    /// When the assignment was proposed.
    pub proposed_at: chrono::DateTime<chrono::Utc>,
}

/// Action to take after assignment logic.
///
/// Represents the result of the assignment decision process.
///
/// # Example
///
/// ```
/// use codirigent_core::{AssignmentAction, TaskId, SessionId};
///
/// // Immediate assignment
/// let action = AssignmentAction::AssignNow {
///     task_id: TaskId::from("task-001"),
///     session_id: SessionId(1),
///     prompt: "Do the task".to_string(),
/// };
///
/// // Check if we should assign
/// if let AssignmentAction::AssignNow { task_id, prompt, .. } = action {
///     println!("Assigning {} with prompt", task_id);
/// }
/// ```
#[derive(Debug, Clone)]
pub enum AssignmentAction {
    /// Assign immediately (auto_assign + no confirmation).
    AssignNow {
        /// The task to assign.
        task_id: TaskId,
        /// The session to assign to.
        session_id: SessionId,
        /// The generated prompt.
        prompt: String,
    },
    /// Wait for user confirmation.
    AwaitConfirmation {
        /// The task to assign.
        task_id: TaskId,
        /// The session to assign to.
        session_id: SessionId,
    },
    /// No task available to assign.
    NoTask,
}

/// Task assignment manager handles routing tasks to sessions.
///
/// The manager tracks pending assignments and decides whether to
/// immediately assign tasks or wait for user confirmation.
///
/// # Thread Safety
///
/// `AssignmentManager` is not thread-safe by itself. For concurrent access,
/// wrap it in an `Arc<Mutex<AssignmentManager>>` or similar synchronization primitive.
///
/// # Example
///
/// ```
/// use codirigent_core::{
///     AssignmentManager, AssignmentConfig, Task, TaskId,
///     DefaultEventBus,
/// };
/// use std::sync::Arc;
///
/// let event_bus = Arc::new(DefaultEventBus::new(16));
/// let config = AssignmentConfig::default();
/// let manager = AssignmentManager::new(config, event_bus);
///
/// // Generate a prompt for a task
/// let task = Task::new(
///     TaskId::from("task-001"),
///     "Fix Bug".to_string(),
///     "The login page crashes".to_string(),
/// );
/// let prompt = manager.generate_prompt(&task);
/// assert!(prompt.contains("Fix Bug"));
/// ```
pub struct AssignmentManager {
    /// Configuration.
    config: AssignmentConfig,

    /// Pending assignments awaiting confirmation.
    pending: Vec<PendingAssignment>,

    /// Event bus for publishing events.
    event_bus: Arc<dyn EventBus>,
}

impl AssignmentManager {
    /// Create a new assignment manager.
    ///
    /// # Arguments
    ///
    /// * `config` - Assignment configuration
    /// * `event_bus` - Event bus for publishing assignment events
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{AssignmentManager, AssignmentConfig, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let manager = AssignmentManager::new(AssignmentConfig::default(), event_bus);
    /// ```
    pub fn new(config: AssignmentConfig, event_bus: Arc<dyn EventBus>) -> Self {
        Self {
            config,
            pending: Vec::new(),
            event_bus,
        }
    }

    /// Get the configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{AssignmentManager, AssignmentConfig, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let manager = AssignmentManager::new(AssignmentConfig::default(), event_bus);
    /// assert!(manager.config().auto_assign);
    /// ```
    pub fn config(&self) -> &AssignmentConfig {
        &self.config
    }

    /// Set whether auto-assign is enabled.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable auto-assign
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{AssignmentManager, AssignmentConfig, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut manager = AssignmentManager::new(AssignmentConfig::default(), event_bus);
    /// assert!(manager.config().auto_assign);
    ///
    /// manager.set_auto_assign(false);
    /// assert!(!manager.config().auto_assign);
    /// ```
    pub fn set_auto_assign(&mut self, enabled: bool) {
        self.config.auto_assign = enabled;
    }

    /// Set whether confirmation is required before auto-assigning.
    ///
    /// When disabling, clears any pending proposals to avoid orphaned assignments.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to require confirmation before auto-assign
    pub fn set_confirm_before_assign(&mut self, enabled: bool) {
        self.config.confirm_before_assign = enabled;
        if !enabled {
            self.pending.clear();
        }
    }

    /// Check if there is a pending assignment for a given session.
    pub fn has_pending_for_session(&self, session_id: SessionId) -> bool {
        self.pending.iter().any(|p| p.session_id == session_id)
    }

    /// Generate prompt for a task.
    ///
    /// Replaces all placeholders in the prompt template with actual task values.
    ///
    /// # Arguments
    ///
    /// * `task` - The task to generate a prompt for
    ///
    /// # Returns
    ///
    /// The generated prompt string with all placeholders replaced.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{
    ///     AssignmentManager, AssignmentConfig, Task, TaskId,
    ///     VerificationConfig, DefaultEventBus,
    /// };
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let manager = AssignmentManager::new(AssignmentConfig::default(), event_bus);
    ///
    /// let mut task = Task::new(
    ///     TaskId::from("task-001"),
    ///     "Fix Auth Bug".to_string(),
    ///     "Login returns 500".to_string(),
    /// );
    /// task.verification = Some(VerificationConfig {
    ///     command: "npm test".to_string(),
    ///     ..Default::default()
    /// });
    ///
    /// let prompt = manager.generate_prompt(&task);
    /// assert!(prompt.contains("Fix Auth Bug"));
    /// assert!(prompt.contains("npm test"));
    /// ```
    pub fn generate_prompt(&self, task: &Task) -> String {
        let verification_cmd = task
            .verification
            .as_ref()
            .map(|v| v.command.as_str())
            .unwrap_or("(auto-detect)");

        let plan_file_text = match (&task.project_dir, &task.plan_file) {
            (Some(dir), Some(file)) => {
                let abs_path = dir.join(file);
                format!("Read and execute the plan at: `{}`", abs_path.display())
            }
            (None, Some(file)) => format!("Read and execute the plan at: `{}`", file),
            _ => "(no plan file)".to_string(),
        };

        self.config
            .prompt_template
            .replace("{title}", &task.title)
            .replace("{id}", &task.id.to_string())
            .replace("{priority}", &format!("{:?}", task.priority))
            .replace("{description}", &task.description)
            .replace("{plan_file}", &plan_file_text)
            .replace("{verification_command}", verification_cmd)
    }

    /// Get all pending assignments.
    ///
    /// # Returns
    ///
    /// A slice of all pending assignments awaiting confirmation.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{AssignmentManager, AssignmentConfig, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let manager = AssignmentManager::new(AssignmentConfig::default(), event_bus);
    /// assert!(manager.pending_assignments().is_empty());
    /// ```
    pub fn pending_assignments(&self) -> &[PendingAssignment] {
        &self.pending
    }

    /// Clear expired pending assignments.
    ///
    /// Removes assignments that have been pending for longer than the specified age.
    ///
    /// # Arguments
    ///
    /// * `max_age_seconds` - Maximum age in seconds before an assignment expires
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{
    ///     AssignmentManager, AssignmentConfig, PendingAssignment,
    ///     TaskId, SessionId, DefaultEventBus,
    /// };
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut manager = AssignmentManager::new(AssignmentConfig::default(), event_bus);
    ///
    /// // Clear assignments older than 60 seconds
    /// manager.clear_expired(60);
    /// ```
    pub fn clear_expired(&mut self, max_age_seconds: u64) {
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(max_age_seconds as i64);

        self.pending.retain(|p| p.proposed_at > cutoff);
    }

    /// Handle a session becoming idle - may trigger assignment.
    ///
    /// This method is called when a session becomes idle and may be available
    /// for a new task. It checks if auto-assign is enabled and if there are
    /// tasks available in the queue.
    ///
    /// # Arguments
    ///
    /// * `session` - The session that became idle
    /// * `queue` - The task queue to get tasks from
    /// * `completed_tasks` - List of completed task IDs for dependency checking
    ///
    /// # Returns
    ///
    /// - `Some(AssignmentAction::AssignNow { .. })` - Task should be assigned immediately
    /// - `Some(AssignmentAction::AwaitConfirmation { .. })` - Task is pending confirmation
    /// - `Some(AssignmentAction::NoTask)` - No tasks available
    /// - `None` - Session not available (e.g., already has a task)
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{
    ///     AssignmentManager, AssignmentConfig, AssignmentAction,
    ///     Task, TaskId, Session, SessionId, TaskQueue, SchedulerConfig,
    ///     DefaultEventBus,
    /// };
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut manager = AssignmentManager::new(AssignmentConfig::default(), event_bus.clone());
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// // Add a task
    /// let task = Task::new(TaskId::from("task-001"), "Task".to_string(), "Desc".to_string());
    /// queue.enqueue(task).unwrap();
    ///
    /// // Session becomes idle
    /// let session = Session::new(SessionId(1), "Session".to_string(), PathBuf::from("/tmp"));
    /// let action = manager.on_session_idle(&session, &mut queue, &[]);
    ///
    /// assert!(matches!(action, Some(AssignmentAction::AssignNow { .. })));
    /// ```
    pub fn on_session_idle(
        &mut self,
        session: &Session,
        queue: &mut TaskQueue,
        completed_tasks: &[TaskId],
    ) -> Option<AssignmentAction> {
        // Check if auto-assign is enabled
        if !self.config.auto_assign {
            return Some(AssignmentAction::NoTask);
        }

        // Check if session already has a task
        if session.current_task.is_some() {
            return None;
        }

        // Get next task for this session
        let task = match queue.next_task_for_session(session, completed_tasks) {
            Some(t) => t,
            None => return Some(AssignmentAction::NoTask),
        };
        let task_id = task.id.clone();
        let prompt = self.generate_prompt(task);

        if self.config.confirm_before_assign {
            // Duplicate proposal guard: skip if we already have a pending entry
            // for the same task or the same session (polling loop protection)
            if self.pending.iter().any(|p| p.task_id == task_id) {
                return Some(AssignmentAction::AwaitConfirmation {
                    task_id,
                    session_id: session.id,
                });
            }
            if self.has_pending_for_session(session.id) {
                return Some(AssignmentAction::AwaitConfirmation {
                    task_id,
                    session_id: session.id,
                });
            }

            // Add to pending and wait for confirmation
            let pending = PendingAssignment {
                task_id: task_id.clone(),
                session_id: session.id,
                prompt: prompt.clone(),
                proposed_at: chrono::Utc::now(),
            };
            self.pending.push(pending);

            // Publish proposal event
            self.event_bus.publish(CodirigentEvent::TaskAssigned {
                task_id: task_id.clone(),
                session_id: session.id,
            });

            Some(AssignmentAction::AwaitConfirmation {
                task_id,
                session_id: session.id,
            })
        } else {
            // Assign immediately
            Some(AssignmentAction::AssignNow {
                task_id,
                session_id: session.id,
                prompt,
            })
        }
    }

    /// Confirm a pending assignment.
    ///
    /// Removes the assignment from pending and publishes a confirmation event.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to confirm
    ///
    /// # Returns
    ///
    /// The confirmed assignment if found.
    ///
    /// # Errors
    ///
    /// Returns an error if no pending assignment exists for the task ID.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{
    ///     AssignmentManager, AssignmentConfig, PendingAssignment,
    ///     TaskId, SessionId, DefaultEventBus,
    /// };
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let config = AssignmentConfig {
    ///     confirm_before_assign: true,
    ///     ..Default::default()
    /// };
    /// let mut manager = AssignmentManager::new(config, event_bus);
    ///
    /// // Try to confirm non-existent assignment
    /// let result = manager.confirm_assignment(&TaskId::from("nonexistent"));
    /// assert!(result.is_err());
    /// ```
    pub fn confirm_assignment(&mut self, task_id: &TaskId) -> Result<PendingAssignment> {
        let idx = self
            .pending
            .iter()
            .position(|p| &p.task_id == task_id)
            .ok_or_else(|| anyhow!("No pending assignment for task {}", task_id))?;

        let assignment = self.pending.remove(idx);

        // Publish confirmation event
        self.event_bus.publish(CodirigentEvent::TaskAssigned {
            task_id: assignment.task_id.clone(),
            session_id: assignment.session_id,
        });

        Ok(assignment)
    }

    /// Reject a pending assignment.
    ///
    /// Removes the assignment from pending and publishes a rejection event.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to reject
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{
    ///     AssignmentManager, AssignmentConfig, PendingAssignment,
    ///     TaskId, SessionId, DefaultEventBus,
    /// };
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut manager = AssignmentManager::new(AssignmentConfig::default(), event_bus);
    ///
    /// // Reject a non-existent assignment (no error, just no-op)
    /// manager.reject_assignment(&TaskId::from("nonexistent"));
    /// ```
    pub fn reject_assignment(&mut self, task_id: &TaskId) {
        if let Some(idx) = self.pending.iter().position(|p| &p.task_id == task_id) {
            // Simply remove — the task was never assigned, it stays Queued
            self.pending.remove(idx);
        }
    }

    /// Find a pending assignment by task ID.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task ID to search for
    ///
    /// # Returns
    ///
    /// The pending assignment if found.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{
    ///     AssignmentManager, AssignmentConfig, TaskId, DefaultEventBus,
    /// };
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let manager = AssignmentManager::new(AssignmentConfig::default(), event_bus);
    ///
    /// assert!(manager.find_pending(&TaskId::from("nonexistent")).is_none());
    /// ```
    pub fn find_pending(&self, task_id: &TaskId) -> Option<&PendingAssignment> {
        self.pending.iter().find(|p| &p.task_id == task_id)
    }
}

impl std::fmt::Debug for AssignmentManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssignmentManager")
            .field("config", &self.config)
            .field("pending_count", &self.pending.len())
            .finish()
    }
}

/// Trait for assignment operations.
///
/// This trait provides a common interface for assignment operations,
/// allowing for different implementations and easier testing.
pub trait AssignmentService: Send + Sync {
    /// Handle session becoming idle.
    fn on_idle(
        &mut self,
        session: &Session,
        queue: &mut TaskQueue,
        completed: &[TaskId],
    ) -> Option<AssignmentAction>;

    /// Confirm assignment and get the prompt.
    fn confirm(&mut self, task_id: &TaskId) -> Result<String>;

    /// Reject assignment.
    fn reject(&mut self, task_id: &TaskId);

    /// Get pending assignments count.
    fn pending_count(&self) -> usize;
}

impl AssignmentService for AssignmentManager {
    fn on_idle(
        &mut self,
        session: &Session,
        queue: &mut TaskQueue,
        completed: &[TaskId],
    ) -> Option<AssignmentAction> {
        self.on_session_idle(session, queue, completed)
    }

    fn confirm(&mut self, task_id: &TaskId) -> Result<String> {
        let assignment = self.confirm_assignment(task_id)?;
        Ok(assignment.prompt)
    }

    fn reject(&mut self, task_id: &TaskId) {
        self.reject_assignment(task_id);
    }

    fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::DefaultEventBus;
    use crate::scheduler::SchedulerConfig;
    use std::path::PathBuf;

    fn create_manager() -> AssignmentManager {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        AssignmentManager::new(AssignmentConfig::default(), event_bus)
    }

    fn create_manager_with_confirmation() -> AssignmentManager {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let config = AssignmentConfig {
            confirm_before_assign: true,
            ..Default::default()
        };
        AssignmentManager::new(config, event_bus)
    }

    fn create_test_setup() -> (AssignmentManager, TaskQueue, Session) {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let manager = AssignmentManager::new(AssignmentConfig::default(), event_bus.clone());

        let queue = TaskQueue::new(SchedulerConfig::default(), event_bus);

        let session = Session::new(
            SessionId(1),
            "Test Session".to_string(),
            PathBuf::from("/test"),
        );

        (manager, queue, session)
    }

    // ========== AssignmentConfig Tests ==========

    #[test]
    fn test_assignment_config_default() {
        let config = AssignmentConfig::default();
        assert!(config.auto_assign);
        assert!(!config.confirm_before_assign);
        assert_eq!(config.idle_threshold_seconds, 5);
        assert!(config.prompt_template.contains("{title}"));
        assert!(config.prompt_template.contains("{description}"));
        assert!(config.prompt_template.contains("{priority}"));
        assert!(config.prompt_template.contains("{verification_command}"));
    }

    #[test]
    fn test_assignment_config_serialization() {
        let config = AssignmentConfig {
            auto_assign: false,
            confirm_before_assign: true,
            idle_threshold_seconds: 10,
            prompt_template: "Custom: {title}".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: AssignmentConfig = serde_json::from_str(&json).unwrap();

        assert!(!parsed.auto_assign);
        assert!(parsed.confirm_before_assign);
        assert_eq!(parsed.idle_threshold_seconds, 10);
        assert_eq!(parsed.prompt_template, "Custom: {title}");
    }

    #[test]
    fn test_assignment_config_equality() {
        let config1 = AssignmentConfig::default();
        let config2 = AssignmentConfig::default();
        let config3 = AssignmentConfig {
            auto_assign: false,
            ..Default::default()
        };

        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_assignment_config_clone() {
        let config = AssignmentConfig::default();
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_assignment_config_debug() {
        let config = AssignmentConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("AssignmentConfig"));
        assert!(debug_str.contains("auto_assign"));
    }

    // ========== DEFAULT_PROMPT_TEMPLATE Tests ==========

    #[test]
    fn test_default_prompt_template_placeholders() {
        assert!(DEFAULT_PROMPT_TEMPLATE.contains("{title}"));
        assert!(DEFAULT_PROMPT_TEMPLATE.contains("{id}"));
        assert!(DEFAULT_PROMPT_TEMPLATE.contains("{priority}"));
        assert!(DEFAULT_PROMPT_TEMPLATE.contains("{description}"));
        assert!(DEFAULT_PROMPT_TEMPLATE.contains("{plan_file}"));
        assert!(DEFAULT_PROMPT_TEMPLATE.contains("{verification_command}"));
    }

    #[test]
    fn test_default_prompt_template_structure() {
        assert!(DEFAULT_PROMPT_TEMPLATE.contains("## Task:"));
        assert!(DEFAULT_PROMPT_TEMPLATE.contains("### Description"));
        assert!(DEFAULT_PROMPT_TEMPLATE.contains("### Plan File"));
        assert!(DEFAULT_PROMPT_TEMPLATE.contains("### Verification"));
    }

    // ========== AssignmentManager Basic Tests ==========

    #[test]
    fn test_assignment_manager_new() {
        let manager = create_manager();
        assert!(manager.pending_assignments().is_empty());
        assert!(manager.config().auto_assign);
    }

    #[test]
    fn test_assignment_manager_config() {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let config = AssignmentConfig {
            auto_assign: false,
            confirm_before_assign: true,
            idle_threshold_seconds: 15,
            prompt_template: "Test".to_string(),
        };
        let manager = AssignmentManager::new(config, event_bus);

        assert!(!manager.config().auto_assign);
        assert!(manager.config().confirm_before_assign);
        assert_eq!(manager.config().idle_threshold_seconds, 15);
    }

    #[test]
    fn test_set_auto_assign() {
        let mut manager = create_manager();
        assert!(manager.config().auto_assign);

        manager.set_auto_assign(false);
        assert!(!manager.config().auto_assign);

        manager.set_auto_assign(true);
        assert!(manager.config().auto_assign);
    }

    #[test]
    fn test_assignment_manager_debug() {
        let manager = create_manager();
        let debug_str = format!("{:?}", manager);
        assert!(debug_str.contains("AssignmentManager"));
        assert!(debug_str.contains("pending_count"));
    }

    // ========== Generate Prompt Tests ==========

    #[test]
    fn test_generate_prompt() {
        let manager = create_manager();

        let mut task = Task::new(
            TaskId::from("task-001"),
            "Fix Auth Bug".to_string(),
            "The login endpoint returns 500 when password is empty.".to_string(),
        );
        task.priority = TaskPriority::High;
        task.verification = Some(VerificationConfig {
            command: "npm test".to_string(),
            ..Default::default()
        });

        let prompt = manager.generate_prompt(&task);

        assert!(prompt.contains("Fix Auth Bug"));
        assert!(prompt.contains("task-001"));
        assert!(prompt.contains("High"));
        assert!(prompt.contains("login endpoint returns 500"));
        assert!(prompt.contains("npm test"));
    }

    #[test]
    fn test_generate_prompt_no_verification() {
        let manager = create_manager();

        let task = Task::new(
            TaskId::from("task-002"),
            "Simple Task".to_string(),
            "Do something".to_string(),
        );

        let prompt = manager.generate_prompt(&task);
        assert!(prompt.contains("(auto-detect)"));
    }

    #[test]
    fn test_generate_prompt_all_priorities() {
        let manager = create_manager();

        let priorities = [
            TaskPriority::Critical,
            TaskPriority::High,
            TaskPriority::Medium,
            TaskPriority::Low,
        ];

        for priority in priorities {
            let mut task = Task::new(
                TaskId::from("task"),
                "Task".to_string(),
                "".to_string(),
            );
            task.priority = priority;

            let prompt = manager.generate_prompt(&task);
            assert!(prompt.contains(&format!("{:?}", priority)));
        }
    }

    #[test]
    fn test_generate_prompt_with_plan_file_and_project_dir() {
        let manager = create_manager();

        let mut task = Task::new(
            TaskId::from("task-001"),
            "Task".to_string(),
            "Desc".to_string(),
        );
        task.project_dir = Some(PathBuf::from("/home/user/project"));
        task.plan_file = Some("plans/phase-1.md".to_string());

        let prompt = manager.generate_prompt(&task);
        // Should contain absolute path
        assert!(prompt.contains("/home/user/project"));
        assert!(prompt.contains("plans/phase-1.md"));
        assert!(prompt.contains("Read and execute the plan at:"));
    }

    #[test]
    fn test_generate_prompt_with_plan_file_no_project_dir() {
        let manager = create_manager();

        let mut task = Task::new(
            TaskId::from("task-001"),
            "Task".to_string(),
            "Desc".to_string(),
        );
        task.plan_file = Some("plans/phase-1.md".to_string());

        let prompt = manager.generate_prompt(&task);
        assert!(prompt.contains("Read and execute the plan at: `plans/phase-1.md`"));
    }

    #[test]
    fn test_generate_prompt_without_plan_file() {
        let manager = create_manager();

        let task = Task::new(
            TaskId::from("task-001"),
            "Task".to_string(),
            "Desc".to_string(),
        );

        let prompt = manager.generate_prompt(&task);
        assert!(prompt.contains("(no plan file)"));
    }

    #[test]
    fn test_generate_prompt_custom_template() {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let config = AssignmentConfig {
            prompt_template: "TASK: {title} | ID: {id}".to_string(),
            ..Default::default()
        };
        let manager = AssignmentManager::new(config, event_bus);

        let task = Task::new(
            TaskId::from("test-id"),
            "Test Title".to_string(),
            "Description".to_string(),
        );

        let prompt = manager.generate_prompt(&task);
        assert_eq!(prompt, "TASK: Test Title | ID: test-id");
    }

    // ========== On Session Idle Tests ==========

    #[test]
    fn test_on_session_idle_no_tasks() {
        let (mut manager, mut queue, session) = create_test_setup();

        let action = manager.on_session_idle(&session, &mut queue, &[]);
        assert!(matches!(action, Some(AssignmentAction::NoTask)));
    }

    #[test]
    fn test_on_session_idle_assigns_task() {
        let (mut manager, mut queue, session) = create_test_setup();

        // Add a task
        let task = Task::new(
            TaskId::from("task-001"),
            "Test Task".to_string(),
            "Description".to_string(),
        );
        queue.enqueue(task).unwrap();

        let action = manager.on_session_idle(&session, &mut queue, &[]);

        match action {
            Some(AssignmentAction::AssignNow {
                task_id,
                session_id,
                prompt,
            }) => {
                assert_eq!(task_id, TaskId::from("task-001"));
                assert_eq!(session_id, SessionId(1));
                assert!(prompt.contains("Test Task"));
            }
            _ => panic!("Expected AssignNow action"),
        }
    }

    #[test]
    fn test_on_session_idle_with_confirmation() {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let config = AssignmentConfig {
            confirm_before_assign: true,
            ..Default::default()
        };
        let mut manager = AssignmentManager::new(config, event_bus.clone());

        let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);

        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/test"));

        let task = Task::new(
            TaskId::from("task-001"),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        let action = manager.on_session_idle(&session, &mut queue, &[]);

        assert!(matches!(
            action,
            Some(AssignmentAction::AwaitConfirmation { .. })
        ));
        assert_eq!(manager.pending.len(), 1);
    }

    #[test]
    fn test_on_session_idle_auto_assign_disabled() {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let config = AssignmentConfig {
            auto_assign: false,
            ..Default::default()
        };
        let mut manager = AssignmentManager::new(config, event_bus.clone());

        let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);

        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/test"));

        let task = Task::new(
            TaskId::from("task-001"),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        let action = manager.on_session_idle(&session, &mut queue, &[]);

        assert!(matches!(action, Some(AssignmentAction::NoTask)));
    }

    #[test]
    fn test_on_session_idle_session_has_task() {
        let (mut manager, mut queue, mut session) = create_test_setup();

        // Session already has a task
        session.current_task = Some(TaskId::from("existing"));

        // Add a task to the queue
        let task = Task::new(
            TaskId::from("task-001"),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        let action = manager.on_session_idle(&session, &mut queue, &[]);

        // Should return None because session is not available
        assert!(action.is_none());
    }

    // ========== Confirm Assignment Tests ==========

    #[test]
    fn test_confirm_assignment() {
        let mut manager = create_manager_with_confirmation();

        // Add a pending assignment manually
        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            prompt: "Test prompt".to_string(),
            proposed_at: chrono::Utc::now(),
        });

        let assignment = manager
            .confirm_assignment(&TaskId::from("task-001"))
            .unwrap();
        assert_eq!(assignment.task_id, TaskId::from("task-001"));
        assert!(manager.pending.is_empty());
    }

    #[test]
    fn test_confirm_nonexistent_fails() {
        let mut manager = create_manager();

        let result = manager.confirm_assignment(&TaskId::from("nonexistent"));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No pending assignment"));
    }

    // ========== Reject Assignment Tests ==========

    #[test]
    fn test_reject_assignment() {
        let mut manager = create_manager();

        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            prompt: "Test".to_string(),
            proposed_at: chrono::Utc::now(),
        });

        manager.reject_assignment(&TaskId::from("task-001"));
        assert!(manager.pending.is_empty());
    }

    #[test]
    fn test_reject_nonexistent() {
        let mut manager = create_manager();

        // Should not panic
        manager.reject_assignment(&TaskId::from("nonexistent"));
        assert!(manager.pending.is_empty());
    }

    // ========== Find Pending Tests ==========

    #[test]
    fn test_find_pending() {
        let mut manager = create_manager();

        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            prompt: "Test".to_string(),
            proposed_at: chrono::Utc::now(),
        });

        let found = manager.find_pending(&TaskId::from("task-001"));
        assert!(found.is_some());
        assert_eq!(found.unwrap().task_id, TaskId::from("task-001"));

        let not_found = manager.find_pending(&TaskId::from("nonexistent"));
        assert!(not_found.is_none());
    }

    // ========== Clear Expired Tests ==========

    #[test]
    fn test_clear_expired() {
        let mut manager = create_manager();

        // Add an old assignment
        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("old-task"),
            session_id: SessionId(1),
            prompt: "Old".to_string(),
            proposed_at: chrono::Utc::now() - chrono::Duration::seconds(120),
        });

        // Add a new assignment
        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("new-task"),
            session_id: SessionId(2),
            prompt: "New".to_string(),
            proposed_at: chrono::Utc::now(),
        });

        // Clear assignments older than 60 seconds
        manager.clear_expired(60);

        assert_eq!(manager.pending.len(), 1);
        assert_eq!(manager.pending[0].task_id, TaskId::from("new-task"));
    }

    #[test]
    fn test_clear_expired_keeps_all_recent() {
        let mut manager = create_manager();

        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("task-1"),
            session_id: SessionId(1),
            prompt: "1".to_string(),
            proposed_at: chrono::Utc::now(),
        });

        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("task-2"),
            session_id: SessionId(2),
            prompt: "2".to_string(),
            proposed_at: chrono::Utc::now(),
        });

        manager.clear_expired(60);

        assert_eq!(manager.pending.len(), 2);
    }

    #[test]
    fn test_clear_expired_removes_all_old() {
        let mut manager = create_manager();

        let old_time = chrono::Utc::now() - chrono::Duration::seconds(120);

        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("task-1"),
            session_id: SessionId(1),
            prompt: "1".to_string(),
            proposed_at: old_time,
        });

        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("task-2"),
            session_id: SessionId(2),
            prompt: "2".to_string(),
            proposed_at: old_time,
        });

        manager.clear_expired(60);

        assert!(manager.pending.is_empty());
    }

    // ========== PendingAssignment Tests ==========

    #[test]
    fn test_pending_assignment_clone() {
        let pending = PendingAssignment {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            prompt: "Test".to_string(),
            proposed_at: chrono::Utc::now(),
        };

        let cloned = pending.clone();
        assert_eq!(cloned.task_id, pending.task_id);
        assert_eq!(cloned.session_id, pending.session_id);
        assert_eq!(cloned.prompt, pending.prompt);
    }

    #[test]
    fn test_pending_assignment_debug() {
        let pending = PendingAssignment {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            prompt: "Test".to_string(),
            proposed_at: chrono::Utc::now(),
        };

        let debug_str = format!("{:?}", pending);
        assert!(debug_str.contains("PendingAssignment"));
        assert!(debug_str.contains("task-001"));
    }

    // ========== AssignmentAction Tests ==========

    #[test]
    fn test_assignment_action_assign_now() {
        let action = AssignmentAction::AssignNow {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            prompt: "Test prompt".to_string(),
        };

        if let AssignmentAction::AssignNow {
            task_id,
            session_id,
            prompt,
        } = action
        {
            assert_eq!(task_id, TaskId::from("task-001"));
            assert_eq!(session_id, SessionId(1));
            assert_eq!(prompt, "Test prompt");
        } else {
            panic!("Expected AssignNow");
        }
    }

    #[test]
    fn test_assignment_action_await_confirmation() {
        let action = AssignmentAction::AwaitConfirmation {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
        };

        assert!(matches!(action, AssignmentAction::AwaitConfirmation { .. }));
    }

    #[test]
    fn test_assignment_action_no_task() {
        let action = AssignmentAction::NoTask;
        assert!(matches!(action, AssignmentAction::NoTask));
    }

    #[test]
    fn test_assignment_action_clone() {
        let action = AssignmentAction::AssignNow {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            prompt: "Test".to_string(),
        };

        let cloned = action.clone();
        assert!(matches!(cloned, AssignmentAction::AssignNow { .. }));
    }

    #[test]
    fn test_assignment_action_debug() {
        let action = AssignmentAction::NoTask;
        let debug_str = format!("{:?}", action);
        assert!(debug_str.contains("NoTask"));
    }

    // ========== AssignmentService Trait Tests ==========

    #[test]
    fn test_assignment_service_on_idle() {
        let (mut manager, mut queue, session) = create_test_setup();

        let task = Task::new(
            TaskId::from("task-001"),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        let action = AssignmentService::on_idle(&mut manager, &session, &mut queue, &[]);
        assert!(matches!(action, Some(AssignmentAction::AssignNow { .. })));
    }

    #[test]
    fn test_assignment_service_confirm() {
        let mut manager = create_manager_with_confirmation();

        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            prompt: "Test prompt".to_string(),
            proposed_at: chrono::Utc::now(),
        });

        let prompt =
            AssignmentService::confirm(&mut manager, &TaskId::from("task-001")).unwrap();
        assert_eq!(prompt, "Test prompt");
    }

    #[test]
    fn test_assignment_service_reject() {
        let mut manager = create_manager();

        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            prompt: "Test".to_string(),
            proposed_at: chrono::Utc::now(),
        });

        AssignmentService::reject(&mut manager, &TaskId::from("task-001"));
        assert_eq!(AssignmentService::pending_count(&manager), 0);
    }

    #[test]
    fn test_assignment_service_pending_count() {
        let mut manager = create_manager();
        assert_eq!(AssignmentService::pending_count(&manager), 0);

        manager.pending.push(PendingAssignment {
            task_id: TaskId::from("task-001"),
            session_id: SessionId(1),
            prompt: "Test".to_string(),
            proposed_at: chrono::Utc::now(),
        });

        assert_eq!(AssignmentService::pending_count(&manager), 1);
    }

    // ========== Integration Tests ==========

    #[test]
    fn test_full_assignment_workflow_immediate() {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let mut manager = AssignmentManager::new(AssignmentConfig::default(), event_bus.clone());
        let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);

        // Create a task
        let mut task = Task::new(
            TaskId::from("task-001"),
            "Implement Feature X".to_string(),
            "Add the X feature to the system".to_string(),
        );
        task.priority = TaskPriority::High;
        task.verification = Some(VerificationConfig {
            command: "cargo test".to_string(),
            ..Default::default()
        });

        queue.enqueue(task).unwrap();

        // Session becomes idle
        let session = Session::new(
            SessionId(1),
            "Session 1".to_string(),
            PathBuf::from("/project"),
        );

        // Get assignment
        let action = manager.on_session_idle(&session, &mut queue, &[]);

        match action {
            Some(AssignmentAction::AssignNow {
                task_id,
                session_id,
                prompt,
            }) => {
                assert_eq!(task_id, TaskId::from("task-001"));
                assert_eq!(session_id, SessionId(1));
                assert!(prompt.contains("Implement Feature X"));
                assert!(prompt.contains("cargo test"));
                assert!(prompt.contains("High"));
            }
            _ => panic!("Expected AssignNow action"),
        }
    }

    #[test]
    fn test_full_assignment_workflow_with_confirmation() {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let config = AssignmentConfig {
            confirm_before_assign: true,
            ..Default::default()
        };
        let mut manager = AssignmentManager::new(config, event_bus.clone());
        let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);

        // Create a task
        let task = Task::new(
            TaskId::from("task-001"),
            "Task".to_string(),
            "Description".to_string(),
        );
        queue.enqueue(task).unwrap();

        // Session becomes idle
        let session = Session::new(SessionId(1), "Session".to_string(), PathBuf::from("/tmp"));

        // Get assignment - should require confirmation
        let action = manager.on_session_idle(&session, &mut queue, &[]);
        assert!(matches!(
            action,
            Some(AssignmentAction::AwaitConfirmation { .. })
        ));
        assert_eq!(manager.pending.len(), 1);

        // Confirm the assignment
        let assignment = manager
            .confirm_assignment(&TaskId::from("task-001"))
            .unwrap();
        assert_eq!(assignment.task_id, TaskId::from("task-001"));
        assert!(manager.pending.is_empty());
    }

    #[test]
    fn test_multiple_pending_assignments() {
        let mut manager = create_manager_with_confirmation();

        // Add multiple pending assignments
        for i in 1..=5 {
            manager.pending.push(PendingAssignment {
                task_id: TaskId(format!("task-{:03}", i)),
                session_id: SessionId(i),
                prompt: format!("Prompt {}", i),
                proposed_at: chrono::Utc::now(),
            });
        }

        assert_eq!(manager.pending.len(), 5);

        // Confirm one
        manager
            .confirm_assignment(&TaskId::from("task-003"))
            .unwrap();
        assert_eq!(manager.pending.len(), 4);

        // Reject one
        manager.reject_assignment(&TaskId::from("task-001"));
        assert_eq!(manager.pending.len(), 3);

        // Remaining should be 002, 004, 005
        let ids: Vec<_> = manager
            .pending
            .iter()
            .map(|p| p.task_id.0.clone())
            .collect();
        assert!(ids.contains(&"task-002".to_string()));
        assert!(ids.contains(&"task-004".to_string()));
        assert!(ids.contains(&"task-005".to_string()));
    }
}
