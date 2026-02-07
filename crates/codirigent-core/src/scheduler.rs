//! Task queue management and scheduling.
//!
//! This module provides the [`TaskQueue`] system for managing task ordering,
//! priority-based scheduling, and dependency tracking. It supports multiple
//! scheduling modes and integrates with the event bus for state change notifications.
//!
//! ## Scheduling Modes
//!
//! - [`SchedulerMode::Fifo`]: First-in, first-out ordering
//! - [`SchedulerMode::Priority`]: Order by priority level (Critical > High > Medium > Low)
//! - [`SchedulerMode::Dependency`]: Consider only dependency ordering
//! - [`SchedulerMode::Smart`]: Combine priority, age, and tag matching (default)
//!
//! ## Example
//!
//! ```
//! use codirigent_core::{
//!     TaskQueue, SchedulerConfig, SchedulerMode,
//!     Task, TaskId, DefaultEventBus,
//! };
//! use codirigent_core::traits::EventBus;
//! use std::sync::Arc;
//!
//! // Create a task queue with default configuration
//! let event_bus = Arc::new(DefaultEventBus::new(16));
//! let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
//!
//! // Add a task
//! let task = Task::new(
//!     TaskId("task-001".to_string()),
//!     "Implement feature".to_string(),
//!     "Add new feature X".to_string(),
//! );
//! queue.enqueue(task).unwrap();
//!
//! // Get the next task to work on
//! if let Some(next) = queue.next_task(&[]) {
//!     println!("Next task: {}", next.title);
//! }
//! ```

use crate::events::CodirigentEvent;
use crate::traits::EventBus;
use crate::types::*;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Check if a session's working_directory is within a task's project_dir.
///
/// Uses canonicalized prefix matching: the session directory must be equal to
/// or a subdirectory of the project directory.
///
/// # Arguments
///
/// * `session_dir` - The session's working directory
/// * `project_dir` - The task's required project directory
///
/// # Returns
///
/// `true` if the session directory is within the project directory.
fn session_matches_project(session_dir: &Path, project_dir: &Path) -> bool {
    let canon_session = std::fs::canonicalize(session_dir)
        .unwrap_or_else(|_| session_dir.to_path_buf());
    let canon_project = std::fs::canonicalize(project_dir)
        .unwrap_or_else(|_| project_dir.to_path_buf());
    canon_session.starts_with(&canon_project)
}

/// Scheduling mode for task assignment.
///
/// Determines how tasks are ordered and selected for assignment to sessions.
///
/// # Example
///
/// ```
/// use codirigent_core::SchedulerMode;
///
/// let mode = SchedulerMode::default();
/// assert_eq!(mode, SchedulerMode::Smart);
///
/// let mode = SchedulerMode::Priority;
/// assert_eq!(mode, SchedulerMode::Priority);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SchedulerMode {
    /// First-in, first-out ordering.
    ///
    /// Tasks are processed in the order they were added to the queue.
    Fifo,

    /// Order by priority level.
    ///
    /// Higher priority tasks are selected first (Critical > High > Medium > Low).
    Priority,

    /// Consider dependencies only.
    ///
    /// Tasks with fewer unmet dependencies are prioritized.
    Dependency,

    /// Combine priority, age, and dependencies (default).
    ///
    /// Uses a weighted scoring system considering priority level,
    /// time spent waiting in queue, and tag matching with sessions.
    #[default]
    Smart,
}

/// Configuration for the task scheduler.
///
/// Controls how tasks are ordered, when they are auto-assigned,
/// and the weighting factors for smart scheduling.
///
/// # Example
///
/// ```
/// use codirigent_core::{SchedulerConfig, SchedulerMode};
///
/// let config = SchedulerConfig::default();
/// assert_eq!(config.mode, SchedulerMode::Smart);
/// assert!(config.auto_assign);
/// assert_eq!(config.idle_threshold_seconds, 5);
///
/// // Custom configuration
/// let config = SchedulerConfig {
///     mode: SchedulerMode::Priority,
///     auto_assign: false,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchedulerConfig {
    /// Scheduling mode determining task ordering.
    pub mode: SchedulerMode,

    /// Whether to auto-assign tasks when sessions become idle.
    pub auto_assign: bool,

    /// Whether to confirm before auto-assigning.
    pub confirm_before_assign: bool,

    /// Seconds of idle time before considering a session available.
    pub idle_threshold_seconds: u32,

    /// Weight for priority in smart mode (0.0-1.0).
    pub priority_weight: f32,

    /// Weight for waiting time in smart mode (0.0-1.0).
    pub age_weight: f32,

    /// Weight for tag matching in smart mode (0.0-1.0).
    pub tag_match_weight: f32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            mode: SchedulerMode::default(),
            auto_assign: true,
            confirm_before_assign: false,
            idle_threshold_seconds: 5,
            priority_weight: 0.5,
            age_weight: 0.3,
            tag_match_weight: 0.2,
        }
    }
}

/// Task queue manager handles task ordering and scheduling.
///
/// The `TaskQueue` manages a collection of tasks, maintaining their order
/// based on the configured scheduling mode. It tracks dependencies between
/// tasks and publishes events when tasks are created, assigned, or completed.
///
/// # Thread Safety
///
/// `TaskQueue` is not thread-safe by itself. For concurrent access,
/// wrap it in an `Arc<Mutex<TaskQueue>>` or similar synchronization primitive.
///
/// # Example
///
/// ```
/// use codirigent_core::{
///     TaskQueue, SchedulerConfig, Task, TaskId, TaskPriority,
///     DefaultEventBus,
/// };
/// use codirigent_core::traits::EventBus;
/// use std::sync::Arc;
///
/// let event_bus = Arc::new(DefaultEventBus::new(16));
/// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
///
/// // Create and enqueue tasks with different priorities
/// let mut high_task = Task::new(
///     TaskId("high".to_string()),
///     "High priority".to_string(),
///     "".to_string(),
/// );
/// high_task.priority = TaskPriority::High;
///
/// let low_task = Task::new(
///     TaskId("low".to_string()),
///     "Low priority".to_string(),
///     "".to_string(),
/// );
///
/// queue.enqueue(low_task).unwrap();
/// queue.enqueue(high_task).unwrap();
///
/// // High priority task should be selected first
/// let next = queue.next_task(&[]).unwrap();
/// assert_eq!(next.id, TaskId("high".to_string()));
/// ```
pub struct TaskQueue {
    /// Queue state (order, blocked tasks).
    state: QueueState,

    /// All tasks indexed by ID.
    tasks: HashMap<TaskId, Task>,

    /// Scheduler configuration.
    config: SchedulerConfig,

    /// Event bus for publishing events.
    event_bus: Arc<dyn EventBus>,
}

impl TaskQueue {
    /// Create a new task queue with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Scheduler configuration
    /// * `event_bus` - Event bus for publishing task events
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    /// ```
    pub fn new(config: SchedulerConfig, event_bus: Arc<dyn EventBus>) -> Self {
        Self {
            state: QueueState::default(),
            tasks: HashMap::new(),
            config,
            event_bus,
        }
    }

    /// Load queue state from storage.
    ///
    /// This should be called after creating a new `TaskQueue` to restore
    /// previously persisted state.
    ///
    /// # Arguments
    ///
    /// * `state` - The queue state to restore
    /// * `tasks` - All tasks to load into the queue
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, QueueState, Task, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// // Load previously saved state
    /// let state = QueueState::default();
    /// let tasks: Vec<Task> = vec![];
    /// queue.load_state(state, tasks);
    /// ```
    pub fn load_state(&mut self, state: QueueState, tasks: Vec<Task>) {
        self.state = state;
        self.tasks = tasks.into_iter().map(|t| (t.id.clone(), t)).collect();
    }

    /// Get current queue state for persistence.
    ///
    /// Returns a reference to the internal queue state which can be
    /// serialized and saved to disk.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    /// let state = queue.get_state();
    /// assert!(state.order.is_empty());
    /// ```
    pub fn get_state(&self) -> &QueueState {
        &self.state
    }

    /// Get the scheduler configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, SchedulerMode, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let config = SchedulerConfig {
    ///     mode: SchedulerMode::Fifo,
    ///     ..Default::default()
    /// };
    /// let queue = TaskQueue::new(config, event_bus);
    /// assert_eq!(queue.config().mode, SchedulerMode::Fifo);
    /// ```
    pub fn config(&self) -> &SchedulerConfig {
        &self.config
    }

    /// Get a task by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The task ID to look up
    ///
    /// # Returns
    ///
    /// The task if found, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, Task, TaskId, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let task = Task::new(TaskId("test".to_string()), "Test".to_string(), "".to_string());
    /// queue.enqueue(task).unwrap();
    ///
    /// assert!(queue.get_task(&TaskId("test".to_string())).is_some());
    /// assert!(queue.get_task(&TaskId("nonexistent".to_string())).is_none());
    /// ```
    pub fn get_task(&self, id: &TaskId) -> Option<&Task> {
        self.tasks.get(id)
    }

    /// Get a mutable reference to a task by ID.
    ///
    /// Used for in-place status updates (e.g., moving to review).
    pub fn get_task_mut(&mut self, id: &TaskId) -> Option<&mut Task> {
        self.tasks.get_mut(id)
    }

    /// Add a task to the queue.
    ///
    /// The task is inserted into the queue order based on the current
    /// scheduling mode. If the task has dependencies, it may be marked
    /// as blocked until those dependencies are satisfied.
    ///
    /// # Arguments
    ///
    /// * `task` - The task to add
    ///
    /// # Errors
    ///
    /// Returns an error if a task with the same ID already exists.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, Task, TaskId, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let task = Task::new(
    ///     TaskId("task-001".to_string()),
    ///     "Test Task".to_string(),
    ///     "Description".to_string(),
    /// );
    ///
    /// queue.enqueue(task).unwrap();
    /// assert_eq!(queue.get_state().order.len(), 1);
    /// ```
    pub fn enqueue(&mut self, task: Task) -> Result<()> {
        let id = task.id.clone();

        // Check for duplicate
        if self.tasks.contains_key(&id) {
            return Err(anyhow!("Task {} already exists", id));
        }

        // Add to tasks map
        self.tasks.insert(id.clone(), task);

        // Add to queue order based on mode
        self.insert_by_priority(&id);

        // Update blocked status
        self.update_blocked_for_task(&id);

        // Update timestamp
        self.state.updated_at = Some(chrono::Utc::now());

        // Publish event
        self.event_bus.publish(CodirigentEvent::TaskCreated { id });

        Ok(())
    }

    /// Remove a task from the queue.
    ///
    /// Removes the task from both the order list and the tasks map.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the task to remove
    ///
    /// # Returns
    ///
    /// The removed task if found, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, Task, TaskId, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let task = Task::new(TaskId("test".to_string()), "Test".to_string(), "".to_string());
    /// queue.enqueue(task).unwrap();
    ///
    /// let removed = queue.dequeue(&TaskId("test".to_string()));
    /// assert!(removed.is_some());
    /// assert!(queue.get_state().order.is_empty());
    /// ```
    pub fn dequeue(&mut self, id: &TaskId) -> Option<Task> {
        // Remove from order
        self.state.order.retain(|t| t != id);

        // Remove from blocked
        self.state.blocked.remove(id);

        // Update timestamp
        self.state.updated_at = Some(chrono::Utc::now());

        // Remove and return task
        self.tasks.remove(id)
    }

    /// Get the next task to assign based on scheduling mode.
    ///
    /// Returns the highest-priority unblocked task that is still in
    /// the `Queued` status and has all dependencies satisfied.
    ///
    /// # Arguments
    ///
    /// * `completed_tasks` - List of task IDs that have been completed
    ///
    /// # Returns
    ///
    /// The next task to assign, or `None` if no tasks are available.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, Task, TaskId, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let task = Task::new(TaskId("test".to_string()), "Test".to_string(), "".to_string());
    /// queue.enqueue(task).unwrap();
    ///
    /// let next = queue.next_task(&[]);
    /// assert!(next.is_some());
    /// assert_eq!(next.unwrap().id, TaskId("test".to_string()));
    /// ```
    pub fn next_task(&self, completed_tasks: &[TaskId]) -> Option<&Task> {
        self.state
            .order
            .iter()
            .filter_map(|id| self.tasks.get(id))
            .filter(|task| {
                task.status == TaskStatus::Queued
                    && !self.is_blocked(&task.id)
                    && task.dependencies_satisfied(completed_tasks)
            })
            .max_by(|a, b| {
                let score_a = self.calculate_score(a, completed_tasks);
                let score_b = self.calculate_score(b, completed_tasks);
                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get the next task for a specific session (considers tag matching).
    ///
    /// Similar to `next_task`, but also considers tag matching between
    /// the session's group and task tags for better assignment.
    ///
    /// # Arguments
    ///
    /// * `session` - The session to find a task for
    /// * `completed_tasks` - List of task IDs that have been completed
    ///
    /// # Returns
    ///
    /// The best matching task for the session, or `None` if no tasks are available.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{
    ///     TaskQueue, SchedulerConfig, Task, TaskId, Session, SessionId,
    ///     DefaultEventBus,
    /// };
    /// use std::sync::Arc;
    /// use std::path::PathBuf;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let mut task = Task::new(TaskId("backend".to_string()), "Backend".to_string(), "".to_string());
    /// task.tags = vec!["backend".to_string()];
    /// queue.enqueue(task).unwrap();
    ///
    /// let mut session = Session::new(SessionId(1), "Backend Session".to_string(), PathBuf::from("/tmp"));
    /// session.group = Some("backend".to_string());
    ///
    /// let next = queue.next_task_for_session(&session, &[]);
    /// assert!(next.is_some());
    /// ```
    pub fn next_task_for_session(
        &self,
        session: &Session,
        completed_tasks: &[TaskId],
    ) -> Option<&Task> {
        self.state
            .order
            .iter()
            .filter_map(|id| self.tasks.get(id))
            .filter(|task| {
                task.status == TaskStatus::Queued
                    && !self.is_blocked(&task.id)
                    && task.dependencies_satisfied(completed_tasks)
                    && task.project_dir.as_ref().map_or(true, |pd| {
                        session_matches_project(&session.working_directory, pd)
                    })
            })
            .max_by(|a, b| {
                let score_a = self.calculate_score_for_session(a, session, completed_tasks);
                let score_b = self.calculate_score_for_session(b, session, completed_tasks);
                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Mark a task as assigned to a session.
    ///
    /// Updates the task status to `Assigned`, records the session ID,
    /// removes it from the queue order, and publishes a `TaskAssigned` event.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The ID of the task to assign
    /// * `session_id` - The session to assign the task to
    ///
    /// # Errors
    ///
    /// Returns an error if the task doesn't exist or is not in `Queued` status.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{
    ///     TaskQueue, SchedulerConfig, Task, TaskId, TaskStatus, SessionId,
    ///     DefaultEventBus,
    /// };
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let task = Task::new(TaskId("test".to_string()), "Test".to_string(), "".to_string());
    /// queue.enqueue(task).unwrap();
    ///
    /// queue.assign_task(&TaskId("test".to_string()), SessionId(1)).unwrap();
    ///
    /// let task = queue.get_task(&TaskId("test".to_string())).unwrap();
    /// assert_eq!(task.status, TaskStatus::Assigned);
    /// assert_eq!(task.assigned_session, Some(SessionId(1)));
    /// ```
    pub fn assign_task(&mut self, task_id: &TaskId, session_id: SessionId) -> Result<()> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("Task {} not found", task_id))?;

        if task.status != TaskStatus::Queued {
            return Err(anyhow!(
                "Task {} is not queued (status: {:?})",
                task_id,
                task.status
            ));
        }

        task.status = TaskStatus::Assigned;
        task.assigned_session = Some(session_id);
        task.assigned_at = Some(chrono::Utc::now());

        // Remove from queue order
        self.state.order.retain(|id| id != task_id);

        // Update timestamp
        self.state.updated_at = Some(chrono::Utc::now());

        // Publish event
        self.event_bus.publish(CodirigentEvent::TaskAssigned {
            task_id: task_id.clone(),
            session_id,
        });

        Ok(())
    }

    /// Mark a task as completed.
    ///
    /// Updates the task status to `Done`, records the completion time,
    /// updates blocked status for dependent tasks, and publishes a
    /// `TaskCompleted` event.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The ID of the task to complete
    /// * `success` - Whether the task completed successfully
    ///
    /// # Errors
    ///
    /// Returns an error if the task doesn't exist.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{
    ///     TaskQueue, SchedulerConfig, Task, TaskId, TaskStatus, SessionId,
    ///     DefaultEventBus,
    /// };
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let task = Task::new(TaskId("test".to_string()), "Test".to_string(), "".to_string());
    /// queue.enqueue(task).unwrap();
    /// queue.assign_task(&TaskId("test".to_string()), SessionId(1)).unwrap();
    ///
    /// queue.complete_task(&TaskId("test".to_string()), true).unwrap();
    ///
    /// let task = queue.get_task(&TaskId("test".to_string())).unwrap();
    /// assert_eq!(task.status, TaskStatus::Done);
    /// assert!(task.completed_at.is_some());
    /// ```
    pub fn complete_task(&mut self, task_id: &TaskId, success: bool) -> Result<()> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("Task {} not found", task_id))?;

        task.status = TaskStatus::Done;
        task.completed_at = Some(chrono::Utc::now());

        // Update blocked status for tasks depending on this one
        self.update_blocked_status_after_completion(task_id);

        // Update timestamp
        self.state.updated_at = Some(chrono::Utc::now());

        // Publish event
        self.event_bus.publish(CodirigentEvent::TaskCompleted {
            task_id: task_id.clone(),
            success,
        });

        Ok(())
    }

    /// Move a task back to queue (for retry).
    ///
    /// Resets the task status to `Queued`, clears assignment info,
    /// increments the retry count, and re-inserts it into the queue.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The ID of the task to requeue
    ///
    /// # Errors
    ///
    /// Returns an error if the task doesn't exist or has exceeded max retries.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{
    ///     TaskQueue, SchedulerConfig, Task, TaskId, TaskStatus, SessionId,
    ///     DefaultEventBus,
    /// };
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let task = Task::new(TaskId("test".to_string()), "Test".to_string(), "".to_string());
    /// queue.enqueue(task).unwrap();
    /// queue.assign_task(&TaskId("test".to_string()), SessionId(1)).unwrap();
    ///
    /// queue.requeue_task(&TaskId("test".to_string())).unwrap();
    ///
    /// let task = queue.get_task(&TaskId("test".to_string())).unwrap();
    /// assert_eq!(task.status, TaskStatus::Queued);
    /// assert_eq!(task.retry.retry_count, 1);
    /// ```
    pub fn requeue_task(&mut self, task_id: &TaskId) -> Result<()> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("Task {} not found", task_id))?;

        if !task.can_retry() {
            return Err(anyhow!("Task {} has exceeded max retries", task_id));
        }

        task.increment_retry();
        task.status = TaskStatus::Queued;
        task.assigned_session = None;
        task.assigned_at = None;

        // Re-add to queue
        self.insert_by_priority(task_id);

        // Update timestamp
        self.state.updated_at = Some(chrono::Utc::now());

        Ok(())
    }

    /// Update blocked status for all tasks.
    ///
    /// Recalculates which tasks are blocked based on their dependencies
    /// and the provided list of completed tasks.
    ///
    /// # Arguments
    ///
    /// * `completed_tasks` - List of task IDs that have been completed
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, Task, TaskId, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let task1 = Task::new(TaskId("first".to_string()), "First".to_string(), "".to_string());
    /// let mut task2 = Task::new(TaskId("second".to_string()), "Second".to_string(), "".to_string());
    /// task2.dependencies = vec![TaskId("first".to_string())];
    ///
    /// queue.enqueue(task1).unwrap();
    /// queue.enqueue(task2).unwrap();
    ///
    /// // task2 is initially blocked
    /// assert!(queue.blocked_tasks().iter().any(|t| t.id == TaskId("second".to_string())));
    ///
    /// // After marking task1 as complete, update blocked status
    /// queue.update_blocked_status(&[TaskId("first".to_string())]);
    /// ```
    pub fn update_blocked_status(&mut self, completed_tasks: &[TaskId]) {
        let task_ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
        for id in task_ids {
            self.update_blocked_for_task(&id);
        }

        // Also check against provided completed list
        for blocking in self.state.blocked.values_mut() {
            blocking.retain(|b| !completed_tasks.contains(b));
        }

        // Remove entries with no blockers
        self.state.blocked.retain(|_, v| !v.is_empty());

        // Update timestamp
        self.state.updated_at = Some(chrono::Utc::now());
    }

    /// Get all queued tasks in order.
    ///
    /// Returns tasks that are in `Queued` status, ordered according
    /// to the queue's current ordering.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, Task, TaskId, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let task = Task::new(TaskId("test".to_string()), "Test".to_string(), "".to_string());
    /// queue.enqueue(task).unwrap();
    ///
    /// let queued = queue.queued_tasks();
    /// assert_eq!(queued.len(), 1);
    /// ```
    pub fn queued_tasks(&self) -> Vec<&Task> {
        self.state
            .order
            .iter()
            .filter_map(|id| self.tasks.get(id))
            .filter(|t| t.status == TaskStatus::Queued)
            .collect()
    }

    /// Get all blocked tasks.
    ///
    /// Returns tasks that have unmet dependencies and cannot be assigned.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, Task, TaskId, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let mut task = Task::new(TaskId("test".to_string()), "Test".to_string(), "".to_string());
    /// task.dependencies = vec![TaskId("nonexistent".to_string())];
    /// queue.enqueue(task).unwrap();
    ///
    /// // Note: Only tasks with existing dependencies in the queue are marked blocked
    /// let blocked = queue.blocked_tasks();
    /// ```
    pub fn blocked_tasks(&self) -> Vec<&Task> {
        self.state
            .blocked
            .keys()
            .filter_map(|id| self.tasks.get(id))
            .collect()
    }

    /// Get all tasks (both queued and non-queued).
    ///
    /// Returns a reference to all tasks managed by this queue.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::{TaskQueue, SchedulerConfig, Task, TaskId, DefaultEventBus};
    /// use std::sync::Arc;
    ///
    /// let event_bus = Arc::new(DefaultEventBus::new(16));
    /// let mut queue = TaskQueue::new(SchedulerConfig::default(), event_bus);
    ///
    /// let task = Task::new(TaskId("test".to_string()), "Test".to_string(), "".to_string());
    /// queue.enqueue(task).unwrap();
    ///
    /// assert_eq!(queue.all_tasks().len(), 1);
    /// ```
    pub fn all_tasks(&self) -> &HashMap<TaskId, Task> {
        &self.tasks
    }

    /// Check if a task is blocked.
    ///
    /// # Arguments
    ///
    /// * `id` - The task ID to check
    ///
    /// # Returns
    ///
    /// `true` if the task has unmet dependencies, `false` otherwise.
    pub fn is_blocked(&self, id: &TaskId) -> bool {
        self.state.blocked.contains_key(id)
    }

    /// Insert a task into the queue order based on priority.
    fn insert_by_priority(&mut self, id: &TaskId) {
        let task = match self.tasks.get(id) {
            Some(t) => t,
            None => return,
        };

        match self.config.mode {
            SchedulerMode::Fifo => {
                self.state.order.push(id.clone());
            }
            SchedulerMode::Priority | SchedulerMode::Smart => {
                // Find insertion point based on priority
                let priority_value = priority_to_value(&task.priority);
                let pos = self.state.order.iter().position(|other_id| {
                    self.tasks
                        .get(other_id)
                        .map(|t| priority_to_value(&t.priority) < priority_value)
                        .unwrap_or(false)
                });

                match pos {
                    Some(p) => self.state.order.insert(p, id.clone()),
                    None => self.state.order.push(id.clone()),
                }
            }
            SchedulerMode::Dependency => {
                self.state.order.push(id.clone());
            }
        }
    }

    /// Update blocked status for a specific task.
    fn update_blocked_for_task(&mut self, id: &TaskId) {
        let task = match self.tasks.get(id) {
            Some(t) => t,
            None => return,
        };

        // Find incomplete dependencies (only those that exist in the queue)
        let blocking: Vec<TaskId> = task
            .dependencies
            .iter()
            .filter(|dep_id| {
                self.tasks
                    .get(*dep_id)
                    .map(|t| t.status != TaskStatus::Done)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        if blocking.is_empty() {
            self.state.blocked.remove(id);
        } else {
            self.state.blocked.insert(id.clone(), blocking);
        }
    }

    /// Update blocked status after a task completion.
    fn update_blocked_status_after_completion(&mut self, completed_id: &TaskId) {
        // Remove completed task from all blocking lists
        for blocking in self.state.blocked.values_mut() {
            blocking.retain(|id| id != completed_id);
        }

        // Remove entries with no blockers
        self.state.blocked.retain(|_, v| !v.is_empty());
    }

    /// Calculate priority score for a task.
    fn calculate_score(&self, task: &Task, _completed_tasks: &[TaskId]) -> f32 {
        match self.config.mode {
            SchedulerMode::Fifo => {
                // Earlier position = higher score
                let pos = self.state.order.iter().position(|id| id == &task.id);
                pos.map(|p| 1.0 / (p as f32 + 1.0)).unwrap_or(0.0)
            }
            SchedulerMode::Priority => priority_to_value(&task.priority) as f32,
            SchedulerMode::Dependency => {
                // Fewer dependencies = higher score
                1.0 / (task.dependencies.len() as f32 + 1.0)
            }
            SchedulerMode::Smart => {
                let priority_score = priority_to_value(&task.priority) as f32 / 4.0;
                let age_score = self.calculate_age_score(task);
                self.config.priority_weight * priority_score + self.config.age_weight * age_score
            }
        }
    }

    /// Calculate score for a task considering session tag matching.
    fn calculate_score_for_session(
        &self,
        task: &Task,
        session: &Session,
        completed_tasks: &[TaskId],
    ) -> f32 {
        let base_score = self.calculate_score(task, completed_tasks);

        // Add tag matching bonus
        let tag_score = if let Some(ref group) = session.group {
            if task.tags.iter().any(|t| t == group) {
                1.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        base_score + self.config.tag_match_weight * tag_score
    }

    /// Calculate age score based on how long the task has been waiting.
    fn calculate_age_score(&self, task: &Task) -> f32 {
        let age = chrono::Utc::now()
            .signed_duration_since(task.created_at)
            .num_minutes();
        // Normalize: 1 hour waiting = 1.0 score
        (age as f32 / 60.0).min(1.0)
    }
}

impl std::fmt::Debug for TaskQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskQueue")
            .field("task_count", &self.tasks.len())
            .field("queue_order_len", &self.state.order.len())
            .field("blocked_count", &self.state.blocked.len())
            .field("config", &self.config)
            .finish()
    }
}

/// Trait for task queue operations.
///
/// This trait provides a common interface for task queue operations,
/// allowing for different implementations and easier testing.
pub trait TaskQueueService: Send + Sync {
    /// Add a task to the queue.
    fn enqueue(&mut self, task: Task) -> Result<()>;

    /// Get next task to assign.
    fn next_task(&self, completed_tasks: &[TaskId]) -> Option<&Task>;

    /// Assign a task to a session.
    fn assign(&mut self, task_id: &TaskId, session_id: SessionId) -> Result<()>;

    /// Complete a task.
    fn complete(&mut self, task_id: &TaskId, success: bool) -> Result<()>;

    /// Requeue a task for retry.
    fn requeue(&mut self, task_id: &TaskId) -> Result<()>;

    /// Get queue state.
    fn state(&self) -> &QueueState;
}

impl TaskQueueService for TaskQueue {
    fn enqueue(&mut self, task: Task) -> Result<()> {
        TaskQueue::enqueue(self, task)
    }

    fn next_task(&self, completed_tasks: &[TaskId]) -> Option<&Task> {
        TaskQueue::next_task(self, completed_tasks)
    }

    fn assign(&mut self, task_id: &TaskId, session_id: SessionId) -> Result<()> {
        self.assign_task(task_id, session_id)
    }

    fn complete(&mut self, task_id: &TaskId, success: bool) -> Result<()> {
        self.complete_task(task_id, success)
    }

    fn requeue(&mut self, task_id: &TaskId) -> Result<()> {
        self.requeue_task(task_id)
    }

    fn state(&self) -> &QueueState {
        self.get_state()
    }
}

/// Convert task priority to numeric value for comparison.
fn priority_to_value(priority: &TaskPriority) -> u8 {
    match priority {
        TaskPriority::Critical => 4,
        TaskPriority::High => 3,
        TaskPriority::Medium => 2,
        TaskPriority::Low => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::DefaultEventBus;
    use std::path::PathBuf;

    fn create_queue() -> TaskQueue {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        TaskQueue::new(SchedulerConfig::default(), event_bus)
    }

    fn create_queue_with_mode(mode: SchedulerMode) -> TaskQueue {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let config = SchedulerConfig {
            mode,
            ..Default::default()
        };
        TaskQueue::new(config, event_bus)
    }

    // ========== SchedulerMode Tests ==========

    #[test]
    fn test_scheduler_mode_default() {
        assert_eq!(SchedulerMode::default(), SchedulerMode::Smart);
    }

    #[test]
    fn test_scheduler_mode_equality() {
        assert_eq!(SchedulerMode::Fifo, SchedulerMode::Fifo);
        assert_eq!(SchedulerMode::Priority, SchedulerMode::Priority);
        assert_eq!(SchedulerMode::Dependency, SchedulerMode::Dependency);
        assert_eq!(SchedulerMode::Smart, SchedulerMode::Smart);
        assert_ne!(SchedulerMode::Fifo, SchedulerMode::Priority);
    }

    #[test]
    fn test_scheduler_mode_serialization() {
        let modes = [
            SchedulerMode::Fifo,
            SchedulerMode::Priority,
            SchedulerMode::Dependency,
            SchedulerMode::Smart,
        ];

        for mode in modes {
            let json = serde_json::to_string(&mode).unwrap();
            let parsed: SchedulerMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, parsed);
        }
    }

    #[test]
    fn test_scheduler_mode_debug() {
        let mode = SchedulerMode::Smart;
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("Smart"));
    }

    #[test]
    fn test_scheduler_mode_clone() {
        let mode = SchedulerMode::Priority;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    // ========== SchedulerConfig Tests ==========

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert_eq!(config.mode, SchedulerMode::Smart);
        assert!(config.auto_assign);
        assert!(!config.confirm_before_assign);
        assert_eq!(config.idle_threshold_seconds, 5);
        assert!((config.priority_weight - 0.5).abs() < f32::EPSILON);
        assert!((config.age_weight - 0.3).abs() < f32::EPSILON);
        assert!((config.tag_match_weight - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn test_scheduler_config_serialization() {
        let config = SchedulerConfig {
            mode: SchedulerMode::Priority,
            auto_assign: false,
            confirm_before_assign: true,
            idle_threshold_seconds: 10,
            priority_weight: 0.7,
            age_weight: 0.2,
            tag_match_weight: 0.1,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SchedulerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mode, SchedulerMode::Priority);
        assert!(!parsed.auto_assign);
        assert!(parsed.confirm_before_assign);
        assert_eq!(parsed.idle_threshold_seconds, 10);
    }

    #[test]
    fn test_scheduler_config_equality() {
        let config1 = SchedulerConfig::default();
        let config2 = SchedulerConfig::default();
        let config3 = SchedulerConfig {
            mode: SchedulerMode::Fifo,
            ..Default::default()
        };
        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_scheduler_config_clone() {
        let config = SchedulerConfig::default();
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_scheduler_config_debug() {
        let config = SchedulerConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("SchedulerConfig"));
        assert!(debug_str.contains("auto_assign"));
    }

    // ========== TaskQueue Basic Tests ==========

    #[test]
    fn test_task_queue_new() {
        let queue = create_queue();
        assert!(queue.get_state().order.is_empty());
        assert!(queue.all_tasks().is_empty());
    }

    #[test]
    fn test_task_queue_config() {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let config = SchedulerConfig {
            mode: SchedulerMode::Fifo,
            ..Default::default()
        };
        let queue = TaskQueue::new(config.clone(), event_bus);
        assert_eq!(queue.config().mode, SchedulerMode::Fifo);
    }

    #[test]
    fn test_task_queue_load_state() {
        let mut queue = create_queue();
        let mut state = QueueState::default();
        state.order.push(TaskId("task-001".to_string()));

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );

        queue.load_state(state, vec![task]);

        assert_eq!(queue.get_state().order.len(), 1);
        assert!(queue.get_task(&TaskId("task-001".to_string())).is_some());
    }

    #[test]
    fn test_task_queue_debug() {
        let queue = create_queue();
        let debug_str = format!("{:?}", queue);
        assert!(debug_str.contains("TaskQueue"));
        assert!(debug_str.contains("task_count"));
    }

    // ========== Enqueue Tests ==========

    #[test]
    fn test_enqueue_task() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );

        queue.enqueue(task).unwrap();

        assert_eq!(queue.get_state().order.len(), 1);
        assert!(queue
            .get_task(&TaskId("task-001".to_string()))
            .is_some());
    }

    #[test]
    fn test_enqueue_duplicate_fails() {
        let mut queue = create_queue();
        let task1 = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        let task2 = Task::new(
            TaskId("task-001".to_string()),
            "Duplicate".to_string(),
            "".to_string(),
        );

        queue.enqueue(task1).unwrap();
        let result = queue.enqueue(task2);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_enqueue_updates_timestamp() {
        let mut queue = create_queue();
        assert!(queue.get_state().updated_at.is_none());

        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        assert!(queue.get_state().updated_at.is_some());
    }

    // ========== Dequeue Tests ==========

    #[test]
    fn test_dequeue_task() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );

        queue.enqueue(task).unwrap();
        let removed = queue.dequeue(&TaskId("task-001".to_string()));

        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, TaskId("task-001".to_string()));
        assert!(queue.get_state().order.is_empty());
        assert!(queue.get_task(&TaskId("task-001".to_string())).is_none());
    }

    #[test]
    fn test_dequeue_nonexistent() {
        let mut queue = create_queue();
        let removed = queue.dequeue(&TaskId("nonexistent".to_string()));
        assert!(removed.is_none());
    }

    #[test]
    fn test_dequeue_removes_from_blocked() {
        let mut queue = create_queue();

        let task1 = Task::new(
            TaskId("task-001".to_string()),
            "First".to_string(),
            "".to_string(),
        );
        let mut task2 = Task::new(
            TaskId("task-002".to_string()),
            "Second".to_string(),
            "".to_string(),
        );
        task2.dependencies = vec![TaskId("task-001".to_string())];

        queue.enqueue(task1).unwrap();
        queue.enqueue(task2).unwrap();

        // task-002 should be blocked
        assert!(queue.is_blocked(&TaskId("task-002".to_string())));

        // Dequeue task-002 should remove it from blocked
        queue.dequeue(&TaskId("task-002".to_string()));
        assert!(!queue.is_blocked(&TaskId("task-002".to_string())));
    }

    // ========== Next Task Tests ==========

    #[test]
    fn test_next_task_empty_queue() {
        let queue = create_queue();
        assert!(queue.next_task(&[]).is_none());
    }

    #[test]
    fn test_next_task_single() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        let next = queue.next_task(&[]);
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, TaskId("task-001".to_string()));
    }

    #[test]
    fn test_next_task_priority_order() {
        let mut queue = create_queue();

        let mut low_task = Task::new(
            TaskId("low".to_string()),
            "Low".to_string(),
            "".to_string(),
        );
        low_task.priority = TaskPriority::Low;

        let mut high_task = Task::new(
            TaskId("high".to_string()),
            "High".to_string(),
            "".to_string(),
        );
        high_task.priority = TaskPriority::High;

        queue.enqueue(low_task).unwrap();
        queue.enqueue(high_task).unwrap();

        let next = queue.next_task(&[]);
        assert_eq!(next.unwrap().id, TaskId("high".to_string()));
    }

    #[test]
    fn test_next_task_critical_priority() {
        let mut queue = create_queue();

        let mut high_task = Task::new(
            TaskId("high".to_string()),
            "High".to_string(),
            "".to_string(),
        );
        high_task.priority = TaskPriority::High;

        let mut critical_task = Task::new(
            TaskId("critical".to_string()),
            "Critical".to_string(),
            "".to_string(),
        );
        critical_task.priority = TaskPriority::Critical;

        queue.enqueue(high_task).unwrap();
        queue.enqueue(critical_task).unwrap();

        let next = queue.next_task(&[]);
        assert_eq!(next.unwrap().id, TaskId("critical".to_string()));
    }

    #[test]
    fn test_next_task_respects_dependencies() {
        let mut queue = create_queue();

        let task1 = Task::new(
            TaskId("task-1".to_string()),
            "First".to_string(),
            "".to_string(),
        );

        let mut task2 = Task::new(
            TaskId("task-2".to_string()),
            "Second".to_string(),
            "".to_string(),
        );
        task2.dependencies = vec![TaskId("task-1".to_string())];

        queue.enqueue(task1).unwrap();
        queue.enqueue(task2).unwrap();

        // task-2 should be blocked, so next should return task-1
        let next = queue.next_task(&[]);
        assert_eq!(next.unwrap().id, TaskId("task-1".to_string()));

        // Assign and complete task-1
        queue.assign_task(&TaskId("task-1".to_string()), SessionId(1)).unwrap();
        queue.complete_task(&TaskId("task-1".to_string()), true).unwrap();

        // After task-1 completes, task-2 is unblocked
        let next = queue.next_task(&[TaskId("task-1".to_string())]);
        assert_eq!(next.unwrap().id, TaskId("task-2".to_string()));
    }

    #[test]
    fn test_next_task_skips_assigned() {
        let mut queue = create_queue();

        let task1 = Task::new(
            TaskId("task-1".to_string()),
            "First".to_string(),
            "".to_string(),
        );
        let task2 = Task::new(
            TaskId("task-2".to_string()),
            "Second".to_string(),
            "".to_string(),
        );

        queue.enqueue(task1).unwrap();
        queue.enqueue(task2).unwrap();

        // Assign first task
        queue.assign_task(&TaskId("task-1".to_string()), SessionId(1)).unwrap();

        // Next should return second task
        let next = queue.next_task(&[]);
        assert_eq!(next.unwrap().id, TaskId("task-2".to_string()));
    }

    // ========== Next Task for Session Tests ==========

    #[test]
    fn test_next_task_for_session_tag_matching() {
        let mut queue = create_queue();

        let mut backend_task = Task::new(
            TaskId("backend".to_string()),
            "Backend Task".to_string(),
            "".to_string(),
        );
        backend_task.tags = vec!["backend".to_string()];

        let mut frontend_task = Task::new(
            TaskId("frontend".to_string()),
            "Frontend Task".to_string(),
            "".to_string(),
        );
        frontend_task.tags = vec!["frontend".to_string()];

        queue.enqueue(backend_task).unwrap();
        queue.enqueue(frontend_task).unwrap();

        // Session with backend group should prefer backend task
        let mut session =
            Session::new(SessionId(1), "Backend".to_string(), PathBuf::from("/tmp"));
        session.group = Some("backend".to_string());

        let next = queue.next_task_for_session(&session, &[]);
        assert_eq!(next.unwrap().id, TaskId("backend".to_string()));
    }

    #[test]
    fn test_next_task_for_session_no_group() {
        let mut queue = create_queue();

        let task = Task::new(
            TaskId("task-1".to_string()),
            "Task".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        // Session without group
        let session = Session::new(SessionId(1), "Session".to_string(), PathBuf::from("/tmp"));

        let next = queue.next_task_for_session(&session, &[]);
        assert!(next.is_some());
    }

    // ========== Assign Task Tests ==========

    #[test]
    fn test_assign_task() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        let task = queue.get_task(&TaskId("task-001".to_string())).unwrap();
        assert_eq!(task.status, TaskStatus::Assigned);
        assert_eq!(task.assigned_session, Some(SessionId(1)));
        assert!(task.assigned_at.is_some());
    }

    #[test]
    fn test_assign_task_removes_from_order() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        assert_eq!(queue.get_state().order.len(), 1);

        queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        assert!(queue.get_state().order.is_empty());
    }

    #[test]
    fn test_assign_task_not_found() {
        let mut queue = create_queue();
        let result = queue.assign_task(&TaskId("nonexistent".to_string()), SessionId(1));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_assign_task_not_queued() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        // Assign once
        queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        // Try to assign again
        let result = queue.assign_task(&TaskId("task-001".to_string()), SessionId(2));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not queued"));
    }

    // ========== Complete Task Tests ==========

    #[test]
    fn test_complete_task() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();
        queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        queue
            .complete_task(&TaskId("task-001".to_string()), true)
            .unwrap();

        let task = queue.get_task(&TaskId("task-001".to_string())).unwrap();
        assert_eq!(task.status, TaskStatus::Done);
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_complete_task_unblocks_dependents() {
        let mut queue = create_queue();

        let task1 = Task::new(
            TaskId("task-1".to_string()),
            "First".to_string(),
            "".to_string(),
        );

        let mut task2 = Task::new(
            TaskId("task-2".to_string()),
            "Second".to_string(),
            "".to_string(),
        );
        task2.dependencies = vec![TaskId("task-1".to_string())];

        queue.enqueue(task1).unwrap();
        queue.enqueue(task2).unwrap();

        // task-2 should be blocked
        assert!(queue.is_blocked(&TaskId("task-2".to_string())));

        // Complete task-1
        queue
            .assign_task(&TaskId("task-1".to_string()), SessionId(1))
            .unwrap();
        queue
            .complete_task(&TaskId("task-1".to_string()), true)
            .unwrap();

        // task-2 should no longer be blocked
        assert!(!queue.is_blocked(&TaskId("task-2".to_string())));
    }

    #[test]
    fn test_complete_task_not_found() {
        let mut queue = create_queue();
        let result = queue.complete_task(&TaskId("nonexistent".to_string()), true);
        assert!(result.is_err());
    }

    // ========== Requeue Task Tests ==========

    #[test]
    fn test_requeue_task() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();
        queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        queue.requeue_task(&TaskId("task-001".to_string())).unwrap();

        let task = queue.get_task(&TaskId("task-001".to_string())).unwrap();
        assert_eq!(task.status, TaskStatus::Queued);
        assert_eq!(task.retry.retry_count, 1);
        assert!(task.assigned_session.is_none());
        assert!(task.assigned_at.is_none());
    }

    #[test]
    fn test_requeue_task_max_retries() {
        let mut queue = create_queue();
        let mut task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        task.retry.retry_count = 3; // Already at max
        queue.enqueue(task).unwrap();
        queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        let result = queue.requeue_task(&TaskId("task-001".to_string()));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max retries"));
    }

    #[test]
    fn test_requeue_task_not_found() {
        let mut queue = create_queue();
        let result = queue.requeue_task(&TaskId("nonexistent".to_string()));
        assert!(result.is_err());
    }

    // ========== Update Blocked Status Tests ==========

    #[test]
    fn test_update_blocked_status() {
        let mut queue = create_queue();

        let task1 = Task::new(
            TaskId("task-1".to_string()),
            "First".to_string(),
            "".to_string(),
        );

        let mut task2 = Task::new(
            TaskId("task-2".to_string()),
            "Second".to_string(),
            "".to_string(),
        );
        task2.dependencies = vec![TaskId("task-1".to_string())];

        queue.enqueue(task1).unwrap();
        queue.enqueue(task2).unwrap();

        // Initially blocked
        assert!(queue.is_blocked(&TaskId("task-2".to_string())));

        // Update with completed task
        queue.update_blocked_status(&[TaskId("task-1".to_string())]);

        // No longer blocked
        assert!(!queue.is_blocked(&TaskId("task-2".to_string())));
    }

    // ========== Queued Tasks Tests ==========

    #[test]
    fn test_queued_tasks() {
        let mut queue = create_queue();

        let task1 = Task::new(
            TaskId("task-1".to_string()),
            "First".to_string(),
            "".to_string(),
        );
        let task2 = Task::new(
            TaskId("task-2".to_string()),
            "Second".to_string(),
            "".to_string(),
        );

        queue.enqueue(task1).unwrap();
        queue.enqueue(task2).unwrap();

        let queued = queue.queued_tasks();
        assert_eq!(queued.len(), 2);

        // Assign one
        queue
            .assign_task(&TaskId("task-1".to_string()), SessionId(1))
            .unwrap();

        let queued = queue.queued_tasks();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, TaskId("task-2".to_string()));
    }

    // ========== Blocked Tasks Tests ==========

    #[test]
    fn test_blocked_tasks() {
        let mut queue = create_queue();

        let task1 = Task::new(
            TaskId("task-1".to_string()),
            "First".to_string(),
            "".to_string(),
        );

        let mut task2 = Task::new(
            TaskId("task-2".to_string()),
            "Second".to_string(),
            "".to_string(),
        );
        task2.dependencies = vec![TaskId("task-1".to_string())];

        queue.enqueue(task1).unwrap();
        queue.enqueue(task2).unwrap();

        let blocked = queue.blocked_tasks();
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].id, TaskId("task-2".to_string()));
    }

    // ========== FIFO Mode Tests ==========

    #[test]
    fn test_fifo_mode_ordering() {
        let mut queue = create_queue_with_mode(SchedulerMode::Fifo);

        let mut high_task = Task::new(
            TaskId("high".to_string()),
            "High".to_string(),
            "".to_string(),
        );
        high_task.priority = TaskPriority::High;

        let low_task = Task::new(
            TaskId("low".to_string()),
            "Low".to_string(),
            "".to_string(),
        );

        // Add low first, then high
        queue.enqueue(low_task).unwrap();
        queue.enqueue(high_task).unwrap();

        // In FIFO mode, low should come first (despite lower priority)
        let next = queue.next_task(&[]);
        assert_eq!(next.unwrap().id, TaskId("low".to_string()));
    }

    // ========== Dependency Mode Tests ==========

    #[test]
    fn test_dependency_mode_ordering() {
        let mut queue = create_queue_with_mode(SchedulerMode::Dependency);

        let task1 = Task::new(
            TaskId("no-deps".to_string()),
            "No deps".to_string(),
            "".to_string(),
        );

        let mut task2 = Task::new(
            TaskId("has-deps".to_string()),
            "Has deps".to_string(),
            "".to_string(),
        );
        task2.dependencies = vec![TaskId("external-1".to_string()), TaskId("external-2".to_string())];

        queue.enqueue(task2).unwrap();
        queue.enqueue(task1).unwrap();

        // Task with no deps should score higher in dependency mode
        // (because the dependencies are external and don't block)
        let next = queue.next_task(&[]);
        assert!(next.is_some());
    }

    // ========== Priority to Value Tests ==========

    #[test]
    fn test_priority_to_value() {
        assert_eq!(priority_to_value(&TaskPriority::Critical), 4);
        assert_eq!(priority_to_value(&TaskPriority::High), 3);
        assert_eq!(priority_to_value(&TaskPriority::Medium), 2);
        assert_eq!(priority_to_value(&TaskPriority::Low), 1);
    }

    // ========== TaskQueueService Trait Tests ==========

    #[test]
    fn test_task_queue_service_enqueue() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );

        TaskQueueService::enqueue(&mut queue, task).unwrap();
        assert_eq!(TaskQueueService::state(&queue).order.len(), 1);
    }

    #[test]
    fn test_task_queue_service_next_task() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        let next = TaskQueueService::next_task(&queue, &[]);
        assert!(next.is_some());
    }

    #[test]
    fn test_task_queue_service_assign() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();

        TaskQueueService::assign(&mut queue, &TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        let task = queue.get_task(&TaskId("task-001".to_string())).unwrap();
        assert_eq!(task.status, TaskStatus::Assigned);
    }

    #[test]
    fn test_task_queue_service_complete() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();
        queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        TaskQueueService::complete(&mut queue, &TaskId("task-001".to_string()), true).unwrap();

        let task = queue.get_task(&TaskId("task-001".to_string())).unwrap();
        assert_eq!(task.status, TaskStatus::Done);
    }

    #[test]
    fn test_task_queue_service_requeue() {
        let mut queue = create_queue();
        let task = Task::new(
            TaskId("task-001".to_string()),
            "Test".to_string(),
            "".to_string(),
        );
        queue.enqueue(task).unwrap();
        queue
            .assign_task(&TaskId("task-001".to_string()), SessionId(1))
            .unwrap();

        TaskQueueService::requeue(&mut queue, &TaskId("task-001".to_string())).unwrap();

        let task = queue.get_task(&TaskId("task-001".to_string())).unwrap();
        assert_eq!(task.status, TaskStatus::Queued);
    }

    // ========== All Tasks Tests ==========

    #[test]
    fn test_all_tasks() {
        let mut queue = create_queue();

        let task1 = Task::new(
            TaskId("task-1".to_string()),
            "First".to_string(),
            "".to_string(),
        );
        let task2 = Task::new(
            TaskId("task-2".to_string()),
            "Second".to_string(),
            "".to_string(),
        );

        queue.enqueue(task1).unwrap();
        queue.enqueue(task2).unwrap();

        let all = queue.all_tasks();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key(&TaskId("task-1".to_string())));
        assert!(all.contains_key(&TaskId("task-2".to_string())));
    }

    // ========== Edge Cases Tests ==========

    #[test]
    fn test_multiple_dependencies() {
        let mut queue = create_queue();

        let task1 = Task::new(
            TaskId("task-1".to_string()),
            "First".to_string(),
            "".to_string(),
        );
        let task2 = Task::new(
            TaskId("task-2".to_string()),
            "Second".to_string(),
            "".to_string(),
        );
        let mut task3 = Task::new(
            TaskId("task-3".to_string()),
            "Third".to_string(),
            "".to_string(),
        );
        task3.dependencies = vec![
            TaskId("task-1".to_string()),
            TaskId("task-2".to_string()),
        ];

        queue.enqueue(task1).unwrap();
        queue.enqueue(task2).unwrap();
        queue.enqueue(task3).unwrap();

        // task-3 should be blocked
        assert!(queue.is_blocked(&TaskId("task-3".to_string())));

        // Complete task-1
        queue
            .assign_task(&TaskId("task-1".to_string()), SessionId(1))
            .unwrap();
        queue
            .complete_task(&TaskId("task-1".to_string()), true)
            .unwrap();

        // task-3 should still be blocked (waiting for task-2)
        assert!(queue.is_blocked(&TaskId("task-3".to_string())));

        // Complete task-2
        queue
            .assign_task(&TaskId("task-2".to_string()), SessionId(1))
            .unwrap();
        queue
            .complete_task(&TaskId("task-2".to_string()), true)
            .unwrap();

        // task-3 should no longer be blocked
        assert!(!queue.is_blocked(&TaskId("task-3".to_string())));
    }

    #[test]
    fn test_external_dependency_not_blocking() {
        let mut queue = create_queue();

        let mut task = Task::new(
            TaskId("task-1".to_string()),
            "Task".to_string(),
            "".to_string(),
        );
        // External dependency (not in queue)
        task.dependencies = vec![TaskId("external".to_string())];

        queue.enqueue(task).unwrap();

        // Should not be blocked since dependency doesn't exist in queue
        assert!(!queue.is_blocked(&TaskId("task-1".to_string())));

        // But should not be selectable without completed_tasks
        let next = queue.next_task(&[]);
        assert!(next.is_none());

        // With external dependency marked complete
        let next = queue.next_task(&[TaskId("external".to_string())]);
        assert!(next.is_some());
    }

    // ========== Project Dir Filter Tests ==========

    #[test]
    fn test_session_matches_project_helper() {
        // Exact match (non-existent paths, falls back to raw comparison)
        assert!(session_matches_project(
            std::path::Path::new("/project"),
            std::path::Path::new("/project"),
        ));

        // Subdirectory
        assert!(session_matches_project(
            std::path::Path::new("/project/src"),
            std::path::Path::new("/project"),
        ));

        // No match
        assert!(!session_matches_project(
            std::path::Path::new("/project-b"),
            std::path::Path::new("/project-a"),
        ));
    }

    #[test]
    fn test_next_task_for_session_project_dir_filter() {
        let mut queue = create_queue();

        let mut task_a = Task::new(
            TaskId("task-a".to_string()),
            "Task A".to_string(),
            "".to_string(),
        );
        task_a.project_dir = Some(PathBuf::from("/project-a"));

        let mut task_b = Task::new(
            TaskId("task-b".to_string()),
            "Task B".to_string(),
            "".to_string(),
        );
        task_b.project_dir = Some(PathBuf::from("/project-b"));

        queue.enqueue(task_a).unwrap();
        queue.enqueue(task_b).unwrap();

        // Session in /project-a should only get task-a
        let session_a = Session::new(
            SessionId(1),
            "Session A".to_string(),
            PathBuf::from("/project-a"),
        );
        let next = queue.next_task_for_session(&session_a, &[]);
        assert_eq!(next.unwrap().id, TaskId("task-a".to_string()));

        // Session in /project-b should only get task-b
        let session_b = Session::new(
            SessionId(2),
            "Session B".to_string(),
            PathBuf::from("/project-b"),
        );
        let next = queue.next_task_for_session(&session_b, &[]);
        assert_eq!(next.unwrap().id, TaskId("task-b".to_string()));
    }

    #[test]
    fn test_next_task_for_session_subdirectory_match() {
        let mut queue = create_queue();

        let mut task = Task::new(
            TaskId("task-1".to_string()),
            "Task".to_string(),
            "".to_string(),
        );
        task.project_dir = Some(PathBuf::from("/project"));

        queue.enqueue(task).unwrap();

        // Session in subdirectory of /project should match
        let session = Session::new(
            SessionId(1),
            "Session".to_string(),
            PathBuf::from("/project/src"),
        );
        let next = queue.next_task_for_session(&session, &[]);
        assert!(next.is_some());
    }

    #[test]
    fn test_next_task_for_session_no_project_dir_matches_any() {
        let mut queue = create_queue();

        let task = Task::new(
            TaskId("task-1".to_string()),
            "Task".to_string(),
            "".to_string(),
        );
        // project_dir is None - should match any session

        queue.enqueue(task).unwrap();

        let session = Session::new(
            SessionId(1),
            "Session".to_string(),
            PathBuf::from("/any/directory"),
        );
        let next = queue.next_task_for_session(&session, &[]);
        assert!(next.is_some());
    }

    #[test]
    fn test_next_task_for_session_project_dir_no_match_skips() {
        let mut queue = create_queue();

        let mut task = Task::new(
            TaskId("task-1".to_string()),
            "Task".to_string(),
            "".to_string(),
        );
        task.project_dir = Some(PathBuf::from("/project-a"));

        queue.enqueue(task).unwrap();

        // Session in completely different directory should get nothing
        let session = Session::new(
            SessionId(1),
            "Session".to_string(),
            PathBuf::from("/project-b"),
        );
        let next = queue.next_task_for_session(&session, &[]);
        assert!(next.is_none());
    }
}
