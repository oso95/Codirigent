//! Task manager for unified task management.
//!
//! This module provides the [`TaskManager`] which integrates all Phase 2
//! task management components (scheduler, context, verification, assignment)
//! with the Session Manager from Phase 1.
//!
//! ## Overview
//!
//! The TaskManager coordinates:
//! - Task queue and scheduling
//! - Context tracking for sessions
//! - Verification of completed tasks
//! - Task assignment to sessions
//!
//! ## Example
//!
//! ```ignore
//! use codirigent_core::{
//!     TaskManager, TaskManagerConfig, Task, TaskId,
//!     DefaultEventBus, FileStorageService,
//! };
//! use std::sync::Arc;
//! use std::path::Path;
//!
//! // Create task manager
//! let storage = Arc::new(FileStorageService::new(Path::new("/project")).unwrap());
//! let event_bus = Arc::new(DefaultEventBus::new(16));
//! let config = TaskManagerConfig::default();
//!
//! let mut manager = TaskManager::new(config, storage, event_bus);
//!
//! // Create a task
//! let task = Task::new(
//!     TaskId("task-001".to_string()),
//!     "Implement feature".to_string(),
//!     "Add new feature X".to_string(),
//! );
//! manager.create_task(task).unwrap();
//! ```

use crate::assignment::{AssignmentAction, AssignmentConfig, AssignmentManager};
use crate::compaction::CompactionConfig;
use crate::context::{ContextConfig, ContextTracker};
use crate::events::CodirigentEvent;
use crate::scheduler::{SchedulerConfig, TaskQueue};
use crate::traits::{EventBus, StorageService};
use crate::types::*;
use crate::verification::{VerificationRunner, VerificationRunnerConfig, VerificationService};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// Unified configuration for task management.
///
/// Combines all component configurations for centralized management.
///
/// # Example
///
/// ```
/// use codirigent_core::TaskManagerConfig;
///
/// let config = TaskManagerConfig::default();
/// assert!(config.assignment.auto_assign);
/// assert!(config.verification.auto_detect);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskManagerConfig {
    /// Scheduler configuration.
    pub scheduler: SchedulerConfig,

    /// Assignment configuration.
    pub assignment: AssignmentConfig,

    /// Verification configuration.
    pub verification: VerificationRunnerConfig,

    /// Context tracking configuration.
    pub context: ContextConfig,

    /// Compaction configuration for auto-compacting before verification.
    pub compaction: CompactionConfig,
}

/// Central task manager coordinating all task operations.
///
/// The `TaskManager` integrates:
/// - [`TaskQueue`] for task scheduling and ordering
/// - [`AssignmentManager`] for task-session assignment
/// - [`VerificationRunner`] for post-completion verification
/// - [`ContextTracker`] for context window monitoring
///
/// # Thread Safety
///
/// `TaskManager` is not thread-safe by itself. For concurrent access,
/// wrap it in an `Arc<Mutex<TaskManager>>` or similar synchronization primitive.
///
/// # Example
///
/// ```ignore
/// use codirigent_core::{
///     TaskManager, TaskManagerConfig, Task, TaskId, Session, SessionId,
///     DefaultEventBus, FileStorageService,
/// };
/// use std::sync::Arc;
///
/// let storage = Arc::new(FileStorageService::new(std::path::Path::new("/project")).unwrap());
/// let event_bus = Arc::new(DefaultEventBus::new(16));
/// let mut manager = TaskManager::new(TaskManagerConfig::default(), storage, event_bus);
///
/// // Create and manage tasks
/// let task = Task::new(TaskId("task-001".to_string()), "Test".to_string(), "".to_string());
/// manager.create_task(task).unwrap();
/// ```
pub struct TaskManager {
    /// Task queue.
    queue: TaskQueue,

    /// Assignment manager.
    assignment: AssignmentManager,

    /// Verification runner.
    verification: VerificationRunner,

    /// Context tracker.
    context: ContextTracker,

    /// Storage service for persistence.
    storage: Arc<dyn StorageService>,

    /// Event bus.
    event_bus: Arc<dyn EventBus>,

    /// Completed task IDs (for dependency checking).
    completed_tasks: Vec<TaskId>,
}

impl TaskManager {
    /// Create a new task manager.
    ///
    /// # Arguments
    ///
    /// * `config` - Unified configuration for all components
    /// * `storage` - Storage service for persistence
    /// * `event_bus` - Event bus for publishing events
    ///
    /// # Example
    ///
    /// ```ignore
    /// use codirigent_core::{
    ///     TaskManager, TaskManagerConfig,
    ///     DefaultEventBus, FileStorageService,
    /// };
    /// use std::sync::Arc;
    ///
    /// let storage = Arc::new(FileStorageService::new(std::path::Path::new("/project")).unwrap());
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let manager = TaskManager::new(TaskManagerConfig::default(), storage, event_bus);
    /// ```
    pub fn new(
        config: TaskManagerConfig,
        storage: Arc<dyn StorageService>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            queue: TaskQueue::new(config.scheduler, event_bus.clone()),
            assignment: AssignmentManager::new(config.assignment, event_bus.clone()),
            verification: VerificationRunner::new(config.verification),
            context: ContextTracker::new(config.context),
            storage,
            event_bus,
            completed_tasks: Vec::new(),
        }
    }

    /// Load state from storage.
    ///
    /// Loads all tasks from storage and rebuilds the queue state.
    ///
    /// # Errors
    ///
    /// Returns an error if loading from storage fails.
    pub async fn load(&mut self) -> Result<()> {
        let task_ids = self.storage.list_task_ids()?;

        for id in task_ids {
            if let Some(task) = self.storage.load_task(&id)? {
                match task.status {
                    TaskStatus::Done => {
                        self.completed_tasks.push(task.id.clone());
                    }
                    TaskStatus::Queued => {
                        self.queue.enqueue(task)?;
                    }
                    _ => {
                        // Re-queue non-completed tasks
                        let mut task = task;
                        task.status = TaskStatus::Queued;
                        self.queue.enqueue(task)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Save state to storage.
    ///
    /// Persists all tasks in the queue to storage.
    ///
    /// # Errors
    ///
    /// Returns an error if saving to storage fails.
    pub async fn save(&self) -> Result<()> {
        for task in self.queue.queued_tasks() {
            self.storage.save_task(task)?;
        }
        Ok(())
    }

    // === Task CRUD Operations ===

    /// Create a new task.
    ///
    /// Saves the task to storage and adds it to the queue.
    ///
    /// # Arguments
    ///
    /// * `task` - The task to create
    ///
    /// # Errors
    ///
    /// Returns an error if saving or enqueueing fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let task = Task::new(
    ///     TaskId("task-001".to_string()),
    ///     "Test Task".to_string(),
    ///     "Description".to_string(),
    /// );
    /// manager.create_task(task).unwrap();
    /// ```
    pub fn create_task(&mut self, task: Task) -> Result<()> {
        // Save to storage
        self.storage.save_task(&task)?;

        // Add to queue
        self.queue.enqueue(task)?;

        Ok(())
    }

    /// Get a task by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The task ID to look up
    ///
    /// # Returns
    ///
    /// The task if found in the queue.
    pub fn get_task(&self, id: &TaskId) -> Option<&Task> {
        self.queue.get_task(id)
    }

    /// List all tasks in the queue.
    ///
    /// Returns all tasks regardless of status.
    pub fn list_tasks(&self) -> Vec<&Task> {
        self.queue.all_tasks().values().collect()
    }

    /// Delete a task.
    ///
    /// Removes the task from both the queue and storage.
    ///
    /// # Arguments
    ///
    /// * `id` - The task ID to delete
    ///
    /// # Errors
    ///
    /// Returns an error if deletion from storage fails.
    pub fn delete_task(&mut self, id: &TaskId) -> Result<()> {
        // Remove from queue
        self.queue.dequeue(id);

        // Delete from storage
        self.storage.delete_task(id)?;

        Ok(())
    }

    /// Update an existing task's editable fields (title, description, priority, plan_file).
    ///
    /// Preserves the task's status, assignment, and other runtime state.
    ///
    /// # Errors
    ///
    /// Returns an error if the task doesn't exist or saving fails.
    pub fn update_task(
        &mut self,
        id: &TaskId,
        title: String,
        description: String,
        priority: TaskPriority,
        plan_file: Option<String>,
        project_dir: Option<std::path::PathBuf>,
    ) -> Result<()> {
        let task = self
            .queue
            .get_task_mut(id)
            .ok_or_else(|| anyhow!("Task {} not found", id))?;

        task.title = title;
        task.description = description;
        task.priority = priority;
        task.plan_file = plan_file;
        task.project_dir = project_dir;

        // Persist
        let task_ref = self
            .queue
            .get_task(id)
            .ok_or_else(|| anyhow!("Task {} not found after update", id))?;
        self.storage.save_task(task_ref)?;
        Ok(())
    }

    /// Get queued tasks.
    ///
    /// Returns tasks with `Queued` status.
    pub fn queued_tasks(&self) -> Vec<&Task> {
        self.queue
            .queued_tasks()
            .into_iter()
            .filter(|t| t.status == TaskStatus::Queued)
            .collect()
    }

    /// Get in-progress tasks.
    ///
    /// Returns tasks with `Assigned`, `Working`, or `Verifying` status.
    pub fn in_progress_tasks(&self) -> Vec<&Task> {
        self.queue
            .all_tasks()
            .values()
            .filter(|t| {
                matches!(
                    t.status,
                    TaskStatus::Assigned | TaskStatus::Working | TaskStatus::Verifying
                )
            })
            .collect()
    }

    /// Get completed task IDs.
    ///
    /// Returns the list of task IDs that have been completed.
    pub fn completed_task_ids(&self) -> &[TaskId] {
        &self.completed_tasks
    }

    // === Assignment Operations ===

    /// Handle session becoming idle.
    ///
    /// Checks if a task should be assigned to the session.
    ///
    /// # Arguments
    ///
    /// * `session` - The session that became idle
    ///
    /// # Returns
    ///
    /// An assignment action indicating what should be done.
    pub fn on_session_idle(&mut self, session: &Session) -> Option<AssignmentAction> {
        self.assignment
            .on_session_idle(session, &mut self.queue, &self.completed_tasks)
    }

    /// Confirm a pending assignment.
    ///
    /// Confirms the assignment and returns the generated prompt.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to confirm assignment for
    /// * `session_id` - The session to assign the task to
    ///
    /// # Returns
    ///
    /// The generated prompt for the task.
    ///
    /// # Errors
    ///
    /// Returns an error if no pending assignment exists or assignment fails.
    pub fn confirm_assignment(
        &mut self,
        task_id: &TaskId,
        session_id: SessionId,
    ) -> Result<String> {
        let pending = self.assignment.confirm_assignment(task_id)?;

        // Update task status in queue
        self.queue.assign_task(task_id, session_id)?;

        Ok(pending.prompt)
    }

    /// Directly assign a task to a session (manual assignment).
    ///
    /// Bypasses the pending assignment mechanism — generates the prompt
    /// and assigns immediately. Use this when the user explicitly clicks
    /// "Assign" on a task card.
    pub fn direct_assign(&mut self, task_id: &TaskId, session_id: SessionId) -> Result<String> {
        let task = self
            .queue
            .get_task(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task {} not found", task_id))?;

        let prompt = self.assignment.generate_prompt(task);

        self.queue.assign_task(task_id, session_id)?;

        Ok(prompt)
    }

    /// Reject a pending assignment.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to reject assignment for
    pub fn reject_assignment(&mut self, task_id: &TaskId) {
        self.assignment.reject_assignment(task_id);
    }

    // === Execution Lifecycle ===

    /// Mark a task as started.
    ///
    /// Called when a session begins working on a task.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task that was started
    ///
    /// # Errors
    ///
    /// Returns an error if the task doesn't exist.
    pub fn start_task(&mut self, task_id: &TaskId) -> Result<()> {
        tracing::info!(?task_id, "Task started");
        // The task status update is handled by the queue on assignment
        Ok(())
    }

    /// Handle task completion signal from session.
    ///
    /// Runs verification if configured and returns the result.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task that was completed
    /// * `working_dir` - The working directory for verification
    ///
    /// # Returns
    ///
    /// The completion result indicating next steps.
    ///
    /// # Errors
    ///
    /// Returns an error if the task doesn't exist or verification fails.
    pub async fn on_task_complete(
        &mut self,
        task_id: &TaskId,
        working_dir: &Path,
    ) -> Result<TaskCompletionResult> {
        let task = self
            .get_task(task_id)
            .ok_or_else(|| anyhow!("Task {} not found", task_id))?
            .clone();

        // Check if verification should run
        let commands =
            task.verification
                .as_ref()
                .map(|v| crate::verification::VerificationCommands {
                    unit: if v.command.is_empty() {
                        None
                    } else {
                        Some(v.command.clone())
                    },
                    ..Default::default()
                });

        let should_verify = self
            .verification
            .should_verify(commands.as_ref(), working_dir);

        if !should_verify {
            return Ok(TaskCompletionResult::NoVerification {
                task_id: task_id.clone(),
            });
        }

        // Run verification
        let result = self
            .verification
            .verify(commands.as_ref(), working_dir)
            .await?;

        if result.passed {
            // Move to review status
            Ok(TaskCompletionResult::ReadyForReview {
                task_id: task_id.clone(),
                result,
            })
        } else if task.can_retry() {
            // Generate retry message
            let message = self.verification.format_failure(
                &result,
                task.retry.retry_count + 1,
                task.retry.max_retries,
            );

            // Requeue task
            self.queue.requeue_task(task_id)?;

            let retry_count = task.retry.retry_count + 1;

            Ok(TaskCompletionResult::NeedsRetry {
                task_id: task_id.clone(),
                message,
                retry_count,
            })
        } else {
            // Max retries exceeded
            let message = format!(
                "Task {} has exceeded maximum retries ({}). Marking as blocked.",
                task_id, task.retry.max_retries
            );

            Ok(TaskCompletionResult::Blocked {
                task_id: task_id.clone(),
                message,
            })
        }
    }

    /// Retry a failed task.
    ///
    /// Generates a new prompt with failure information.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to retry
    ///
    /// # Returns
    ///
    /// The retry prompt.
    ///
    /// # Errors
    ///
    /// Returns an error if the task doesn't exist.
    pub fn retry_task(&mut self, task_id: &TaskId) -> Result<String> {
        let task = self
            .get_task(task_id)
            .ok_or_else(|| anyhow!("Task {} not found", task_id))?;

        let prompt = self.assignment.generate_prompt(task);
        Ok(prompt)
    }

    /// Move a task to review status.
    ///
    /// Transitions a task from InProgress/Working to Review, indicating
    /// it needs human review before completion.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to move to review
    ///
    /// # Errors
    ///
    /// Returns an error if the task doesn't exist.
    pub fn move_to_review(&mut self, task_id: &TaskId) -> Result<()> {
        let task = self
            .queue
            .get_task_mut(task_id)
            .ok_or_else(|| anyhow!("Task {} not found", task_id))?;
        task.status = TaskStatus::Review;

        // Persist the updated task
        let task_ref = self
            .queue
            .get_task(task_id)
            .ok_or_else(|| anyhow!("Task {} not found after update", task_id))?;
        self.storage.save_task(task_ref)?;
        Ok(())
    }

    /// Mark a task as reviewed and done.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to approve
    ///
    /// # Errors
    ///
    /// Returns an error if the task doesn't exist or completion fails.
    pub fn approve_task(&mut self, task_id: &TaskId) -> Result<()> {
        self.queue.complete_task(task_id, true)?;
        self.completed_tasks.push(task_id.clone());

        // Save updated task
        if let Some(task) = self.get_task(task_id) {
            self.storage.save_task(task)?;
        }

        Ok(())
    }

    // === Context Tracking ===

    /// Update context usage for a session.
    ///
    /// Publishes an event if a threshold is crossed.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to update
    /// * `usage` - The raw context usage (0.0-1.0)
    pub fn update_context(&mut self, session_id: SessionId, usage: f32) {
        if let Some(event) = self.context.update_usage(session_id, usage) {
            self.event_bus.publish(event);
        }
    }

    /// Get context usage for a session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to query
    ///
    /// # Returns
    ///
    /// The effective context usage if available.
    pub fn get_context(&self, session_id: SessionId) -> Option<f32> {
        self.context
            .get_usage(session_id)
            .map(|u| u.effective_usage)
    }

    // === Accessors ===

    /// Get the task queue.
    pub fn queue(&self) -> &TaskQueue {
        &self.queue
    }

    /// Get a mutable reference to the task queue.
    pub fn queue_mut(&mut self) -> &mut TaskQueue {
        &mut self.queue
    }

    /// Get the assignment manager.
    pub fn assignment(&self) -> &AssignmentManager {
        &self.assignment
    }

    /// Get a mutable reference to the assignment manager.
    pub fn assignment_mut(&mut self) -> &mut AssignmentManager {
        &mut self.assignment
    }

    /// Get the verification runner.
    pub fn verification(&self) -> &VerificationRunner {
        &self.verification
    }

    /// Get the context tracker.
    pub fn context_tracker(&self) -> &ContextTracker {
        &self.context
    }

    /// Get a mutable reference to the context tracker.
    pub fn context_tracker_mut(&mut self) -> &mut ContextTracker {
        &mut self.context
    }

    // === Session-Task Synchronization ===

    /// Find a task assigned to a specific session.
    ///
    /// Returns the task ID and a reference to the task.
    pub fn find_task_by_session(&self, session_id: SessionId) -> Option<(TaskId, &Task)> {
        self.queue
            .all_tasks()
            .iter()
            .find(|(_, task)| task.assigned_session == Some(session_id))
            .map(|(id, task)| (id.clone(), task))
    }

    /// Find a task assigned to a specific session (mutable).
    ///
    /// Returns the task ID of the task assigned to the session.
    fn find_task_id_by_session(&self, session_id: SessionId) -> Option<TaskId> {
        self.queue
            .all_tasks()
            .iter()
            .find(|(_, task)| task.assigned_session == Some(session_id))
            .map(|(id, _)| id.clone())
    }

    /// Handle session status change and automatically sync task status.
    ///
    /// This is the main entry point for unified task-session status tracking.
    /// Called when a session's status changes (via SessionStatusChanged event).
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session that changed status
    /// * `old_status` - The previous session status
    /// * `new_status` - The new session status
    ///
    /// # Returns
    ///
    /// The task ID if a task was updated, None otherwise.
    pub fn on_session_status_changed(
        &mut self,
        session_id: SessionId,
        old_status: SessionStatus,
        new_status: SessionStatus,
    ) -> Option<TaskId> {
        // Find task assigned to this session
        let task_id = self.find_task_id_by_session(session_id)?;

        // Get current task status
        let task = self.queue.get_task(&task_id)?;
        let current_task_status = task.status;

        // Determine if task status should change
        let new_task_status =
            self.map_session_to_task_status(old_status, new_status, current_task_status)?;

        // Update task status
        let reason = format!(
            "Session {} transitioned from {:?} to {:?}",
            session_id, old_status, new_status
        );

        if let Err(e) = self.transition_task_status(&task_id, new_task_status, Some(reason)) {
            tracing::error!(
                ?task_id,
                ?session_id,
                error = %e,
                "Failed to transition task status"
            );
            return None;
        }

        Some(task_id)
    }

    /// Map session status transition to task status change.
    ///
    /// Implements the status mapping rules:
    /// - Session Idle/NeedsAttention → Working (task Assigned) → Task Working
    /// - Session Working → Idle/NeedsAttention (task Working) → Task Review
    /// - Session Error (task Working/Verifying) → Task Blocked
    ///
    /// # Returns
    ///
    /// The new task status, or None if no change is needed.
    fn map_session_to_task_status(
        &self,
        old_session: SessionStatus,
        new_session: SessionStatus,
        current_task: TaskStatus,
    ) -> Option<TaskStatus> {
        match (old_session, new_session, current_task) {
            // Session started working on an assigned task
            (
                SessionStatus::Idle | SessionStatus::NeedsAttention,
                SessionStatus::Working,
                TaskStatus::Assigned,
            ) => {
                tracing::info!("Session started working, transitioning task Assigned → Working");
                Some(TaskStatus::Working)
            }

            // Attention prompts don't change task status (must come before finished-working)
            (_, SessionStatus::NeedsAttention, _) => {
                tracing::debug!("Session needs attention, task status unchanged");
                None
            }

            // Session finished working (returned to idle)
            (SessionStatus::Working, SessionStatus::Idle, TaskStatus::Working) => {
                tracing::info!("Session finished working, transitioning task Working → Review");
                Some(TaskStatus::Review)
            }

            // Session encountered an error
            (_, SessionStatus::Error, TaskStatus::Working | TaskStatus::Verifying) => {
                tracing::warn!("Session encountered error, transitioning task to Blocked");
                Some(TaskStatus::Blocked)
            }

            // All other transitions don't affect task status
            _ => None,
        }
    }

    /// Transition a task to a new status and emit event.
    ///
    /// Updates task status, timestamps, persists to storage, and publishes
    /// a TaskStatusChanged event.
    pub fn transition_task_status(
        &mut self,
        task_id: &TaskId,
        new_status: TaskStatus,
        reason: Option<String>,
    ) -> Result<()> {
        let task = self
            .queue
            .get_task_mut(task_id)
            .ok_or_else(|| anyhow!("Task {} not found", task_id))?;

        let old_status = task.status;

        // Don't transition if already in the target status
        if old_status == new_status {
            return Ok(());
        }

        task.status = new_status;

        // Update timestamps based on status
        match new_status {
            TaskStatus::Working => {
                if task.started_at.is_none() {
                    task.started_at = Some(chrono::Utc::now());
                }
            }
            TaskStatus::Done | TaskStatus::Blocked => {
                if task.completed_at.is_none() {
                    task.completed_at = Some(chrono::Utc::now());
                }
            }
            _ => {}
        }

        // Persist to storage
        let task_ref = self
            .queue
            .get_task(task_id)
            .ok_or_else(|| anyhow!("Task {} not found after update", task_id))?;
        self.storage.save_task(task_ref)?;

        // Emit TaskStatusChanged event
        self.event_bus.publish(CodirigentEvent::TaskStatusChanged {
            task_id: task_id.clone(),
            old: old_status,
            new: new_status,
            reason,
        });

        tracing::info!(
            ?task_id,
            ?old_status,
            ?new_status,
            "Task status transitioned"
        );

        Ok(())
    }
}

impl std::fmt::Debug for TaskManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskManager")
            .field("queue", &self.queue)
            .field("assignment", &self.assignment)
            .field("completed_tasks_count", &self.completed_tasks.len())
            .finish()
    }
}

/// Result of task completion handling.
///
/// Represents the outcome after processing a task completion signal.
#[derive(Debug)]
pub enum TaskCompletionResult {
    /// Verification passed, ready for review.
    ReadyForReview {
        /// The task ID.
        task_id: TaskId,
        /// The verification result.
        result: crate::verification::VerificationResult,
    },
    /// Verification failed, needs retry.
    NeedsRetry {
        /// The task ID.
        task_id: TaskId,
        /// The retry message with failure details.
        message: String,
        /// Current retry count.
        retry_count: u32,
    },
    /// Max retries exceeded, task blocked.
    Blocked {
        /// The task ID.
        task_id: TaskId,
        /// The blocking message.
        message: String,
    },
    /// No verification configured, proceed directly.
    NoVerification {
        /// The task ID.
        task_id: TaskId,
    },
}

/// Trait for coordinated task management.
///
/// Provides a simplified interface for task management operations.
pub trait TaskManagementService: Send + Sync {
    /// Create a new task.
    fn create(&mut self, task: Task) -> Result<()>;

    /// Assign a task to a session.
    fn assign_to_session(&mut self, session: &Session) -> Option<AssignmentAction>;

    /// Approve a completed task.
    fn approve(&mut self, task_id: &TaskId) -> Result<()>;

    /// List tasks by status.
    fn list_by_status(&self, status: TaskStatus) -> Vec<&Task>;
}

impl TaskManagementService for TaskManager {
    fn create(&mut self, task: Task) -> Result<()> {
        self.create_task(task)
    }

    fn assign_to_session(&mut self, session: &Session) -> Option<AssignmentAction> {
        self.on_session_idle(session)
    }

    fn approve(&mut self, task_id: &TaskId) -> Result<()> {
        self.approve_task(task_id)
    }

    fn list_by_status(&self, status: TaskStatus) -> Vec<&Task> {
        self.list_tasks()
            .into_iter()
            .filter(|t| t.status == status)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::DefaultEventBus;
    use crate::storage::FileStorageService;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_task_manager() -> (TaskManager, TempDir) {
        let temp = TempDir::new().unwrap();
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let config = TaskManagerConfig::default();

        let manager = TaskManager::new(config, storage, event_bus);
        (manager, temp)
    }

    // ========== TaskManagerConfig Tests ==========

    #[test]
    fn test_task_manager_config_default() {
        let config = TaskManagerConfig::default();
        assert!(config.assignment.auto_assign);
        assert!(config.verification.auto_detect);
        assert_eq!(config.verification.max_retries, 3);
    }

    #[test]
    fn test_task_manager_config_serialization() {
        let config = TaskManagerConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: TaskManagerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.scheduler.mode, config.scheduler.mode);
        assert_eq!(parsed.assignment.auto_assign, config.assignment.auto_assign);
    }

    #[test]
    fn test_task_manager_config_clone() {
        let config = TaskManagerConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.scheduler.mode, config.scheduler.mode);
    }

    #[test]
    fn test_task_manager_config_debug() {
        let config = TaskManagerConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("TaskManagerConfig"));
        assert!(debug_str.contains("scheduler"));
    }

    // ========== TaskManager Basic Tests ==========

    #[test]
    fn test_task_manager_creation() {
        let (manager, _temp) = create_task_manager();
        assert!(manager.queued_tasks().is_empty());
        assert!(manager.completed_task_ids().is_empty());
    }

    #[test]
    fn test_task_manager_debug() {
        let (manager, _temp) = create_task_manager();
        let debug_str = format!("{:?}", manager);
        assert!(debug_str.contains("TaskManager"));
        assert!(debug_str.contains("queue"));
    }

    // ========== Task CRUD Tests ==========

    #[test]
    fn test_create_task() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test Task".to_string(),
            "Description".to_string(),
        );

        manager.create_task(task).unwrap();
        assert_eq!(manager.queued_tasks().len(), 1);
    }

    #[test]
    fn test_get_task() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        let retrieved = manager.get_task(&TaskId("task-001".to_string()));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test");
    }

    #[test]
    fn test_get_task_not_found() {
        let (manager, _temp) = create_task_manager();
        let retrieved = manager.get_task(&TaskId("nonexistent".to_string()));
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_list_tasks() {
        let (mut manager, _temp) = create_task_manager();

        let task1 = Task::new(
            TaskId("task-001".to_string()),
            "Task 1".to_string(),
            "".to_string(),
        );
        let task2 = Task::new(
            TaskId("task-002".to_string()),
            "Task 2".to_string(),
            "".to_string(),
        );

        manager.create_task(task1).unwrap();
        manager.create_task(task2).unwrap();

        assert_eq!(manager.list_tasks().len(), 2);
    }

    #[test]
    fn test_delete_task() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();
        assert_eq!(manager.queued_tasks().len(), 1);

        manager
            .delete_task(&TaskId("task-001".to_string()))
            .unwrap();
        assert!(manager.queued_tasks().is_empty());
    }

    #[test]
    fn test_queued_tasks() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        let queued = manager.queued_tasks();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].status, TaskStatus::Queued);
    }

    #[test]
    fn test_in_progress_tasks() {
        let (manager, _temp) = create_task_manager();
        // No in-progress tasks initially
        assert!(manager.in_progress_tasks().is_empty());
    }

    // ========== Assignment Tests ==========

    #[test]
    fn test_on_session_idle() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        let session = Session::new(
            SessionId(1),
            "Test Session".to_string(),
            PathBuf::from("/test"),
        );

        let action = manager.on_session_idle(&session);
        assert!(matches!(action, Some(AssignmentAction::AssignNow { .. })));
    }

    #[test]
    fn test_on_session_idle_no_tasks() {
        let (mut manager, _temp) = create_task_manager();

        let session = Session::new(
            SessionId(1),
            "Test Session".to_string(),
            PathBuf::from("/test"),
        );

        let action = manager.on_session_idle(&session);
        assert!(matches!(action, Some(AssignmentAction::NoTask)));
    }

    #[test]
    fn test_reject_assignment() {
        let (mut manager, _temp) = create_task_manager();

        // Should not panic even if no pending assignment
        manager.reject_assignment(&TaskId("nonexistent".to_string()));
    }

    // ========== Lifecycle Tests ==========

    #[test]
    fn test_start_task() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        let result = manager.start_task(&TaskId("task-001".to_string()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_approve_task() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        // Assign first
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/test"));
        if let Some(AssignmentAction::AssignNow { task_id, .. }) = manager.on_session_idle(&session)
        {
            manager.queue.assign_task(&task_id, SessionId(1)).unwrap();
        }

        // Approve
        manager
            .approve_task(&TaskId("task-001".to_string()))
            .unwrap();

        assert!(manager
            .completed_task_ids()
            .contains(&TaskId("task-001".to_string())));
    }

    #[test]
    fn test_retry_task() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test Task".to_string(),
            "Task description".to_string(),
        );
        manager.create_task(task).unwrap();

        let prompt = manager.retry_task(&TaskId("task-001".to_string())).unwrap();
        assert!(prompt.contains("Test Task"));
    }

    #[test]
    fn test_retry_task_not_found() {
        let (mut manager, _temp) = create_task_manager();

        let result = manager.retry_task(&TaskId("nonexistent".to_string()));
        assert!(result.is_err());
    }

    // ========== Context Tracking Tests ==========

    #[test]
    fn test_update_context() {
        let (mut manager, _temp) = create_task_manager();

        manager.update_context(SessionId(1), 0.65);

        let usage = manager.get_context(SessionId(1));
        assert!(usage.is_some());
        assert!((usage.unwrap() - 0.65).abs() < 0.01);
    }

    #[test]
    fn test_get_context_not_found() {
        let (manager, _temp) = create_task_manager();
        assert!(manager.get_context(SessionId(999)).is_none());
    }

    #[test]
    fn test_context_threshold_event() {
        let (mut manager, _temp) = create_task_manager();

        // Start at normal
        manager.update_context(SessionId(1), 0.5);

        // Cross warning threshold - event should be published
        manager.update_context(SessionId(1), 0.75);

        let usage = manager.get_context(SessionId(1));
        assert!((usage.unwrap() - 0.75).abs() < 0.01);
    }

    // ========== Accessor Tests ==========

    #[test]
    fn test_queue_accessor() {
        let (manager, _temp) = create_task_manager();
        let _queue = manager.queue();
        // Just verify it compiles and returns correctly
    }

    #[test]
    fn test_queue_mut_accessor() {
        let (mut manager, _temp) = create_task_manager();
        let _queue = manager.queue_mut();
    }

    #[test]
    fn test_assignment_accessor() {
        let (manager, _temp) = create_task_manager();
        let _assignment = manager.assignment();
    }

    #[test]
    fn test_verification_accessor() {
        let (manager, _temp) = create_task_manager();
        let _verification = manager.verification();
    }

    #[test]
    fn test_context_tracker_accessor() {
        let (manager, _temp) = create_task_manager();
        let _tracker = manager.context_tracker();
    }

    // ========== TaskManagementService Trait Tests ==========

    #[test]
    fn test_service_create() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );

        TaskManagementService::create(&mut manager, task).unwrap();
        assert_eq!(manager.queued_tasks().len(), 1);
    }

    #[test]
    fn test_service_assign_to_session() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/test"));

        let action = TaskManagementService::assign_to_session(&mut manager, &session);
        assert!(matches!(action, Some(AssignmentAction::AssignNow { .. })));
    }

    #[test]
    fn test_service_approve() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        // Assign first
        manager
            .queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        TaskManagementService::approve(&mut manager, &TaskId("task-001".to_string())).unwrap();
        assert!(manager
            .completed_task_ids()
            .contains(&TaskId("task-001".to_string())));
    }

    #[test]
    fn test_service_list_by_status() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        let queued = TaskManagementService::list_by_status(&manager, TaskStatus::Queued);
        assert_eq!(queued.len(), 1);

        let working = TaskManagementService::list_by_status(&manager, TaskStatus::Working);
        assert!(working.is_empty());
    }

    // ========== TaskCompletionResult Tests ==========

    #[test]
    fn test_task_completion_result_ready_for_review() {
        let result = TaskCompletionResult::ReadyForReview {
            task_id: TaskId("task-001".to_string()),
            result: crate::verification::VerificationResult::passed(
                crate::verification::VerificationCheckType::UnitTest,
                1000,
            ),
        };
        assert!(matches!(
            result,
            TaskCompletionResult::ReadyForReview { .. }
        ));
    }

    #[test]
    fn test_task_completion_result_needs_retry() {
        let result = TaskCompletionResult::NeedsRetry {
            task_id: TaskId("task-001".to_string()),
            message: "Test failed".to_string(),
            retry_count: 1,
        };
        if let TaskCompletionResult::NeedsRetry { retry_count, .. } = result {
            assert_eq!(retry_count, 1);
        } else {
            panic!("Expected NeedsRetry");
        }
    }

    #[test]
    fn test_task_completion_result_blocked() {
        let result = TaskCompletionResult::Blocked {
            task_id: TaskId("task-001".to_string()),
            message: "Max retries exceeded".to_string(),
        };
        assert!(matches!(result, TaskCompletionResult::Blocked { .. }));
    }

    #[test]
    fn test_task_completion_result_no_verification() {
        let result = TaskCompletionResult::NoVerification {
            task_id: TaskId("task-001".to_string()),
        };
        assert!(matches!(
            result,
            TaskCompletionResult::NoVerification { .. }
        ));
    }

    #[test]
    fn test_task_completion_result_debug() {
        let result = TaskCompletionResult::NoVerification {
            task_id: TaskId("task-001".to_string()),
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("NoVerification"));
        assert!(debug_str.contains("task-001"));
    }

    // ========== Async Tests ==========

    #[tokio::test]
    async fn test_load_empty() {
        let (mut manager, _temp) = create_task_manager();

        let result = manager.load().await;
        assert!(result.is_ok());
        assert!(manager.queued_tasks().is_empty());
    }

    #[tokio::test]
    async fn test_save_empty() {
        let (manager, _temp) = create_task_manager();

        let result = manager.save().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_load_with_tasks() {
        let temp = TempDir::new().unwrap();
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());
        let event_bus = Arc::new(DefaultEventBus::new(16));

        // Create and save a task
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        storage.save_task(&task).unwrap();

        // Load task manager
        let config = TaskManagerConfig::default();
        let mut manager = TaskManager::new(config, storage, event_bus);

        manager.load().await.unwrap();
        assert_eq!(manager.queued_tasks().len(), 1);
    }

    #[tokio::test]
    async fn test_on_task_complete_no_verification() {
        let temp = TempDir::new().unwrap();
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());
        let event_bus = Arc::new(DefaultEventBus::new(16));

        let mut config = TaskManagerConfig::default();
        config.verification.auto_detect = false;

        let mut manager = TaskManager::new(config, storage, event_bus);

        // Task without verification
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        let result = manager
            .on_task_complete(&TaskId("task-001".to_string()), temp.path())
            .await;
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap(),
            TaskCompletionResult::NoVerification { .. }
        ));
    }

    #[tokio::test]
    async fn test_on_task_complete_not_found() {
        let (mut manager, temp) = create_task_manager();

        let result = manager
            .on_task_complete(&TaskId("nonexistent".to_string()), temp.path())
            .await;
        assert!(result.is_err());
    }

    // ========== Session-Task Synchronization Tests ==========

    #[test]
    fn test_find_task_by_session() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        // Assign to session
        manager
            .queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(42))
            .unwrap();

        // Find by session
        let result = manager.find_task_by_session(SessionId(42));
        assert!(result.is_some());
        let (task_id, _task) = result.unwrap();
        assert_eq!(task_id, TaskId("task-001".to_string()));

        // Not found for different session
        assert!(manager.find_task_by_session(SessionId(99)).is_none());
    }

    #[test]
    fn test_session_working_transitions_assigned_to_working() {
        let (mut manager, _temp) = create_task_manager();

        // Create and assign task
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();
        manager
            .queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        // Simulate session starting work
        let result = manager.on_session_status_changed(
            SessionId(1),
            SessionStatus::Idle,
            SessionStatus::Working,
        );

        assert!(result.is_some());
        assert_eq!(result.unwrap(), TaskId("task-001".to_string()));

        // Verify task status changed
        let task = manager.get_task(&TaskId("task-001".to_string())).unwrap();
        assert_eq!(task.status, TaskStatus::Working);
        assert!(task.started_at.is_some());
    }

    #[test]
    fn test_session_idle_transitions_working_to_review() {
        let (mut manager, _temp) = create_task_manager();

        // Create and assign task
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        // Manually set to Working
        manager.create_task(task).unwrap();
        manager
            .queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        // Update task status to Working via queue
        let task_mut = manager
            .queue
            .get_task_mut(&TaskId("task-001".to_string()))
            .unwrap();
        task_mut.status = TaskStatus::Working;

        // Simulate session finishing work (returned to idle)
        let result = manager.on_session_status_changed(
            SessionId(1),
            SessionStatus::Working,
            SessionStatus::Idle,
        );

        assert!(result.is_some());
        assert_eq!(result.unwrap(), TaskId("task-001".to_string()));

        // Verify task status changed
        let task = manager.get_task(&TaskId("task-001".to_string())).unwrap();
        assert_eq!(task.status, TaskStatus::Review);
    }

    #[test]
    fn test_session_error_transitions_to_blocked() {
        let (mut manager, _temp) = create_task_manager();

        // Create and assign task
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();
        manager
            .queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        // Update task status to Working
        let task_mut = manager
            .queue
            .get_task_mut(&TaskId("task-001".to_string()))
            .unwrap();
        task_mut.status = TaskStatus::Working;

        // Simulate session encountering error
        let result = manager.on_session_status_changed(
            SessionId(1),
            SessionStatus::Working,
            SessionStatus::Error,
        );

        assert!(result.is_some());
        assert_eq!(result.unwrap(), TaskId("task-001".to_string()));

        // Verify task status changed
        let task = manager.get_task(&TaskId("task-001".to_string())).unwrap();
        assert_eq!(task.status, TaskStatus::Blocked);
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_no_task_assigned_does_nothing() {
        let (mut manager, _temp) = create_task_manager();

        // No task assigned to session 99
        let result = manager.on_session_status_changed(
            SessionId(99),
            SessionStatus::Idle,
            SessionStatus::Working,
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_permission_prompt_no_status_change() {
        let (mut manager, _temp) = create_task_manager();

        // Create and assign task
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();
        manager
            .queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        // Simulate session needing attention
        let result = manager.on_session_status_changed(
            SessionId(1),
            SessionStatus::Working,
            SessionStatus::NeedsAttention,
        );

        // Should not change task status
        assert!(result.is_none());
    }

    #[test]
    fn test_transition_task_status_idempotent() {
        let (mut manager, _temp) = create_task_manager();

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        manager.create_task(task).unwrap();

        // Transition to same status should be no-op
        let result = manager.transition_task_status(
            &TaskId("task-001".to_string()),
            TaskStatus::Queued,
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_map_session_to_task_status_all_cases() {
        let (manager, _temp) = create_task_manager();

        // Assigned → Working
        assert_eq!(
            manager.map_session_to_task_status(
                SessionStatus::Idle,
                SessionStatus::Working,
                TaskStatus::Assigned
            ),
            Some(TaskStatus::Working)
        );

        // Working → Review
        assert_eq!(
            manager.map_session_to_task_status(
                SessionStatus::Working,
                SessionStatus::Idle,
                TaskStatus::Working
            ),
            Some(TaskStatus::Review)
        );

        // Error → Blocked
        assert_eq!(
            manager.map_session_to_task_status(
                SessionStatus::Working,
                SessionStatus::Error,
                TaskStatus::Working
            ),
            Some(TaskStatus::Blocked)
        );

        // Attention prompt → No change
        assert_eq!(
            manager.map_session_to_task_status(
                SessionStatus::Working,
                SessionStatus::NeedsAttention,
                TaskStatus::Working
            ),
            None
        );

        // Irrelevant transitions → No change
        assert_eq!(
            manager.map_session_to_task_status(
                SessionStatus::Idle,
                SessionStatus::Idle,
                TaskStatus::Queued
            ),
            None
        );
    }
}
