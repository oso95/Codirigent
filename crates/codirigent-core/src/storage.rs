//! File-based storage service implementation.
//!
//! This module provides the [`FileStorageService`] struct which implements
//! the [`StorageService`] trait for persisting application state to disk.
//!
//! ## Directory Structure
//!
//! All data is stored in the `.codirigent` directory:
//!
//! ```text
//! .codirigent/
//! ├── config.json    # Project configuration
//! ├── state.json     # Runtime state (sessions, layout)
//! ├── queue.json     # Task queue order
//! ├── tasks/         # Individual task files
//! │   ├── task-001.json
//! │   └── task-002.json
//! └── context/       # Per-session context files (written by CLI hooks)
//!     ├── session-1.json
//!     └── session-2.json
//! ```
//!
//! ## Atomic Writes
//!
//! All write operations use atomic writes (write to temp file, then rename)
//! to prevent corruption in case of crashes or power failures.

use crate::traits::StorageService;
use crate::types::{AppState, SessionId, Task, TaskId};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, instrument};

/// Context data written by CLI hooks to a session's context file.
///
/// Hook scripts (Claude Code, Gemini CLI) write this JSON to
/// `.codirigent/context/session-{id}.json`. Codirigent polls these
/// files to track context window usage.
///
/// # Example
///
/// ```
/// use codirigent_core::storage::ContextFileData;
///
/// let data = ContextFileData {
///     usage: 0.42,
///     tokens_used: Some(84000),
///     tokens_total: Some(200000),
/// };
/// let json = serde_json::to_string(&data).unwrap();
/// assert!(json.contains("0.42"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFileData {
    /// Context usage ratio (0.0-1.0). Required.
    pub usage: f32,

    /// Raw token count used. Optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<u64>,

    /// Total token capacity. Optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_total: Option<u64>,
}

/// File-based implementation of [`StorageService`].
///
/// Stores all data in the `.codirigent` directory as JSON files
/// for portability and debuggability.
///
/// # Example
///
/// ```no_run
/// use codirigent_core::storage::FileStorageService;
/// use codirigent_core::traits::StorageService;
/// use std::path::Path;
///
/// let storage = FileStorageService::new(Path::new("/path/to/project")).unwrap();
/// let state = storage.load_state().unwrap();
/// ```
pub struct FileStorageService {
    /// Path to the .codirigent directory.
    codirigent_dir: PathBuf,
}

impl FileStorageService {
    /// Create a new storage service for the given project directory.
    ///
    /// Creates the `.codirigent` directory and its subdirectories if they don't exist.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - The project root directory where `.codirigent` will be created.
    ///
    /// # Errors
    ///
    /// Returns an error if the directories cannot be created.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_core::storage::FileStorageService;
    /// use std::path::Path;
    ///
    /// let storage = FileStorageService::new(Path::new("/path/to/project")).unwrap();
    /// ```
    #[instrument(skip_all, fields(project_dir = %project_dir.display()))]
    pub fn new(project_dir: &Path) -> Result<Self> {
        let codirigent_dir = project_dir.join(".codirigent");
        debug!("Creating storage service at {}", codirigent_dir.display());
        let service = Self { codirigent_dir };
        service.ensure_directories()?;
        Ok(service)
    }

    /// Create a storage service from an existing `.codirigent` directory.
    ///
    /// This is useful when you already have the `.codirigent` path directly.
    ///
    /// # Arguments
    ///
    /// * `codirigent_dir` - The path to the `.codirigent` directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directories cannot be created.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_core::storage::FileStorageService;
    /// use std::path::PathBuf;
    ///
    /// let storage = FileStorageService::from_codirigent_dir(
    ///     PathBuf::from("/path/to/project/.codirigent")
    /// ).unwrap();
    /// ```
    #[instrument(skip_all, fields(codirigent_dir = %codirigent_dir.display()))]
    pub fn from_codirigent_dir(codirigent_dir: PathBuf) -> Result<Self> {
        debug!(
            "Creating storage service from existing dir: {}",
            codirigent_dir.display()
        );
        let service = Self { codirigent_dir };
        service.ensure_directories()?;
        Ok(service)
    }

    /// Ensure all required directories exist.
    ///
    /// Creates the `.codirigent` directory and the `tasks` subdirectory
    /// if they don't already exist. This operation is idempotent.
    fn ensure_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.codirigent_dir).with_context(|| {
            format!(
                "Failed to create .codirigent directory at {:?}",
                self.codirigent_dir
            )
        })?;
        fs::create_dir_all(self.tasks_dir()).with_context(|| {
            format!("Failed to create tasks directory at {:?}", self.tasks_dir())
        })?;
        fs::create_dir_all(self.context_dir()).with_context(|| {
            format!(
                "Failed to create context directory at {:?}",
                self.context_dir()
            )
        })?;
        debug!("Directories ensured at {}", self.codirigent_dir.display());
        Ok(())
    }

    /// Get the path to `state.json`.
    fn state_path(&self) -> PathBuf {
        self.codirigent_dir.join("state.json")
    }

    /// Get the path to the `tasks` directory.
    fn tasks_dir(&self) -> PathBuf {
        self.codirigent_dir.join("tasks")
    }

    /// Get the path to the `context` directory.
    ///
    /// This directory holds per-session context files written by CLI hooks.
    pub fn context_dir(&self) -> PathBuf {
        self.codirigent_dir.join("context")
    }

    /// Get the path to a specific session's context file.
    ///
    /// Context files are named `session-{id}.json` in the context directory.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID
    pub fn context_file(&self, session_id: SessionId) -> PathBuf {
        self.context_dir()
            .join(format!("session-{}.json", session_id.0))
    }

    /// Get the path to a specific task file.
    ///
    /// Task files are named `{task_id}.json` in the tasks directory.
    /// Uses whitelist sanitization: only alphanumeric, dash, and underscore
    /// characters are allowed. All other characters (including null bytes,
    /// path separators, and control characters) are replaced with underscore.
    fn task_path(&self, id: &TaskId) -> PathBuf {
        // Sanitize: only allow alphanumeric, dash, underscore (whitelist approach)
        let safe_id: String =
            id.0.chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
        self.tasks_dir().join(format!("{}.json", safe_id))
    }

    /// Write data atomically to a file.
    ///
    /// Writes to a temporary file first, then renames to the target path.
    /// This prevents corruption in case of crashes during write.
    /// If rename fails after write succeeds, the temp file is cleaned up.
    fn atomic_write(&self, path: &Path, content: &str) -> Result<()> {
        let temp_path = path.with_extension("json.tmp");
        fs::write(&temp_path, content)
            .with_context(|| format!("Failed to write temp file at {:?}", temp_path))?;

        if let Err(e) = fs::rename(&temp_path, path) {
            // Clean up temp file if rename fails
            let _ = fs::remove_file(&temp_path);
            return Err(e).with_context(|| format!("Failed to rename temp file to {:?}", path));
        }
        Ok(())
    }
}

impl StorageService for FileStorageService {
    /// Get the `.codirigent` directory path.
    fn codirigent_dir(&self) -> &Path {
        &self.codirigent_dir
    }

    /// Load application state from disk.
    ///
    /// Returns a default state if `state.json` doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    #[instrument(skip(self))]
    fn load_state(&self) -> Result<AppState> {
        let path = self.state_path();
        if !path.exists() {
            debug!("No state.json found, returning default state");
            return Ok(AppState::default());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read state.json at {:?}", path))?;
        let state: AppState = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse state.json at {:?}", path))?;
        debug!("Loaded state with {} sessions", state.sessions.len());
        Ok(state)
    }

    /// Save application state to disk.
    ///
    /// Updates the `updated_at` timestamp and writes atomically.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or writing fails.
    #[instrument(skip(self, state), fields(sessions = state.sessions.len()))]
    fn save_state(&self, state: &AppState) -> Result<()> {
        let path = self.state_path();
        let mut state = state.clone();
        state.updated_at = Some(chrono::Utc::now());

        let content = serde_json::to_string_pretty(&state).context("Failed to serialize state")?;

        self.atomic_write(&path, &content)?;
        debug!("Saved state to {}", path.display());
        Ok(())
    }

    /// Load a specific task by ID.
    ///
    /// Returns `None` if the task file doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    #[instrument(skip(self), fields(task_id = %id))]
    fn load_task(&self, id: &TaskId) -> Result<Option<Task>> {
        let path = self.task_path(id);
        if !path.exists() {
            debug!("Task file not found: {}", path.display());
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read task file at {:?}", path))?;
        let task: Task = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse task file at {:?}", path))?;
        debug!("Loaded task: {}", id);
        Ok(Some(task))
    }

    /// Save a task to disk.
    ///
    /// Creates or updates the task file using atomic writes.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or writing fails.
    #[instrument(skip(self, task), fields(task_id = %task.id))]
    fn save_task(&self, task: &Task) -> Result<()> {
        let path = self.task_path(&task.id);
        let content = serde_json::to_string_pretty(task).context("Failed to serialize task")?;

        self.atomic_write(&path, &content)?;
        debug!("Saved task to {}", path.display());
        Ok(())
    }

    /// List all task IDs in the tasks directory.
    ///
    /// Returns task IDs sorted alphabetically.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the directory fails.
    #[instrument(skip(self))]
    fn list_task_ids(&self) -> Result<Vec<TaskId>> {
        let tasks_dir = self.tasks_dir();
        if !tasks_dir.exists() {
            debug!("Tasks directory does not exist, returning empty list");
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        for entry in fs::read_dir(&tasks_dir)
            .with_context(|| format!("Failed to read tasks directory at {:?}", tasks_dir))?
        {
            let entry = entry.with_context(|| "Failed to read directory entry")?;
            let path = entry.path();

            // Only process .json files (not .tmp files)
            if path.extension().is_some_and(|e| e == "json") {
                if let Some(stem) = path.file_stem() {
                    if let Some(name) = stem.to_str() {
                        ids.push(TaskId(name.to_string()));
                    }
                }
            }
        }

        ids.sort_by(|a, b| a.0.cmp(&b.0));
        debug!("Found {} tasks", ids.len());
        Ok(ids)
    }

    /// Delete a task from disk.
    ///
    /// Does nothing if the task file doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be deleted.
    #[instrument(skip(self), fields(task_id = %id))]
    fn delete_task(&self, id: &TaskId) -> Result<()> {
        let path = self.task_path(id);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete task file at {:?}", path))?;
            debug!("Deleted task: {}", id);
        } else {
            debug!("Task file not found for deletion: {}", id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        LayoutMode, RetryConfig, Session, SessionId, SessionStatus, TaskPriority, TaskStatus,
    };
    use tempfile::TempDir;

    // Helper function to create a test task
    fn create_test_task(id: &str, title: &str) -> Task {
        Task {
            id: TaskId(id.to_string()),
            title: title.to_string(),
            description: format!("Description for {}", title),
            priority: TaskPriority::Medium,
            status: TaskStatus::Queued,
            dependencies: vec![],
            tags: vec!["test".to_string()],
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

    // ========== Constructor Tests ==========

    #[test]
    fn test_new_creates_directories() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        assert!(storage.codirigent_dir.exists());
        assert!(storage.tasks_dir().exists());
        assert!(storage.context_dir().exists());
    }

    #[test]
    fn test_new_creates_dirigent_subdirectory() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let expected = temp.path().join(".codirigent");
        assert_eq!(storage.codirigent_dir, expected);
    }

    #[test]
    fn test_from_codirigent_dir() {
        let temp = TempDir::new().unwrap();
        let dirigent_path = temp.path().join(".codirigent");
        let storage = FileStorageService::from_codirigent_dir(dirigent_path.clone()).unwrap();

        assert_eq!(storage.codirigent_dir, dirigent_path);
        assert!(storage.codirigent_dir.exists());
        assert!(storage.tasks_dir().exists());
    }

    #[test]
    fn test_ensure_directories_idempotent() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Call ensure_directories again - should not fail
        assert!(storage.ensure_directories().is_ok());
        assert!(storage.codirigent_dir.exists());
        assert!(storage.tasks_dir().exists());
    }

    #[test]
    fn test_codirigent_dir_accessor() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let expected = temp.path().join(".codirigent");
        assert_eq!(storage.codirigent_dir(), expected.as_path());
    }

    // ========== Path Helper Tests ==========

    #[test]
    fn test_state_path() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let expected = temp.path().join(".codirigent").join("state.json");
        assert_eq!(storage.state_path(), expected);
    }

    #[test]
    fn test_tasks_dir() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let expected = temp.path().join(".codirigent").join("tasks");
        assert_eq!(storage.tasks_dir(), expected);
    }

    #[test]
    fn test_task_path() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let task_id = TaskId("task-001".to_string());
        let expected = temp
            .path()
            .join(".codirigent")
            .join("tasks")
            .join("task-001.json");
        assert_eq!(storage.task_path(&task_id), expected);
    }

    #[test]
    fn test_task_path_sanitizes_dangerous_characters() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Task IDs with path traversal attempts should be sanitized
        let dangerous_id = TaskId("../../../etc/passwd".to_string());
        let path = storage.task_path(&dangerous_id);

        // Should not contain path traversal - dots and slashes replaced with underscores
        assert!(!path.to_string_lossy().contains(".."));
        assert!(!path.to_string_lossy().contains("/etc/"));
        assert!(path.starts_with(storage.tasks_dir()));

        // Verify the sanitized filename uses whitelist approach
        // "../../../etc/passwd" -> "_________etc_passwd" (dots and slashes become underscores)
        let filename = path.file_stem().unwrap().to_str().unwrap();
        assert_eq!(filename, "_________etc_passwd");
    }

    #[test]
    fn test_task_path_whitelist_sanitization() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Test various dangerous characters are sanitized
        let test_cases = [
            // (input, expected_sanitized_stem)
            ("valid-task_123", "valid-task_123"), // Valid chars unchanged
            ("task/with/slashes", "task_with_slashes"), // Slashes sanitized
            ("task\\backslash", "task_backslash"), // Backslashes sanitized
            ("task..dots", "task__dots"),         // Dots sanitized
            ("task\0null", "task_null"),          // Null bytes sanitized
            ("task\nwith\ttabs", "task_with_tabs"), // Control chars sanitized
            ("task with spaces", "task_with_spaces"), // Spaces sanitized
            ("task:colon", "task_colon"),         // Colons sanitized
            ("task<>|", "task___"),               // Special chars sanitized
        ];

        for (input, expected) in test_cases {
            let id = TaskId(input.to_string());
            let path = storage.task_path(&id);
            let stem = path.file_stem().unwrap().to_str().unwrap();
            assert_eq!(
                stem, expected,
                "Input '{}' should sanitize to '{}' but got '{}'",
                input, expected, stem
            );
            // All paths should be within tasks_dir
            assert!(path.starts_with(storage.tasks_dir()));
        }
    }

    #[test]
    fn test_task_path_preserves_valid_characters() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Valid task IDs should be preserved exactly
        let valid_id = TaskId("task-001_feature_ABC123".to_string());
        let path = storage.task_path(&valid_id);
        let filename = path.file_name().unwrap().to_str().unwrap();

        assert_eq!(filename, "task-001_feature_ABC123.json");
    }

    // ========== State Load/Save Tests ==========

    #[test]
    fn test_load_state_returns_default_when_missing() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let state = storage.load_state().unwrap();
        assert!(state.sessions.is_empty());
        assert!(matches!(
            state.layout,
            LayoutMode::Grid { rows: 2, cols: 2 }
        ));
    }

    #[test]
    fn test_save_and_load_state() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let mut state = AppState::default();
        state.sessions.push(Session::new(
            SessionId(1),
            "Test Session".to_string(),
            PathBuf::from("/tmp"),
        ));
        state.layout = LayoutMode::Grid { rows: 3, cols: 3 };

        storage.save_state(&state).unwrap();
        let loaded = storage.load_state().unwrap();

        assert_eq!(loaded.sessions.len(), 1);
        assert_eq!(loaded.sessions[0].id, SessionId(1));
        assert_eq!(loaded.sessions[0].name, "Test Session");
        assert!(matches!(
            loaded.layout,
            LayoutMode::Grid { rows: 3, cols: 3 }
        ));
        assert!(loaded.updated_at.is_some());
    }

    #[test]
    fn test_save_state_updates_timestamp() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let state = AppState::default();
        assert!(state.updated_at.is_none());

        storage.save_state(&state).unwrap();
        let loaded = storage.load_state().unwrap();

        assert!(loaded.updated_at.is_some());
    }

    #[test]
    fn test_save_state_creates_file() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let state = AppState::default();
        storage.save_state(&state).unwrap();

        assert!(storage.state_path().exists());
    }

    #[test]
    fn test_save_state_overwrites_existing() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Save initial state
        let mut state1 = AppState::default();
        state1.sessions.push(Session::new(
            SessionId(1),
            "First".to_string(),
            PathBuf::from("/first"),
        ));
        storage.save_state(&state1).unwrap();

        // Save different state
        let mut state2 = AppState::default();
        state2.sessions.push(Session::new(
            SessionId(2),
            "Second".to_string(),
            PathBuf::from("/second"),
        ));
        storage.save_state(&state2).unwrap();

        // Load and verify
        let loaded = storage.load_state().unwrap();
        assert_eq!(loaded.sessions.len(), 1);
        assert_eq!(loaded.sessions[0].id, SessionId(2));
        assert_eq!(loaded.sessions[0].name, "Second");
    }

    #[test]
    fn test_state_with_multiple_sessions() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let mut state = AppState::default();
        for i in 1..=5 {
            state.sessions.push(Session::new(
                SessionId(i),
                format!("Session {}", i),
                PathBuf::from(format!("/project{}", i)),
            ));
        }

        storage.save_state(&state).unwrap();
        let loaded = storage.load_state().unwrap();

        assert_eq!(loaded.sessions.len(), 5);
        for (i, session) in loaded.sessions.iter().enumerate() {
            assert_eq!(session.id, SessionId((i + 1) as u64));
        }
    }

    #[test]
    fn test_state_with_session_all_fields() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let mut session = Session::new(
            SessionId(1),
            "Full Session".to_string(),
            PathBuf::from("/home/user/project"),
        );
        session.status = SessionStatus::Working;
        session.current_task = Some(TaskId("task-001".to_string()));
        session.context_usage = Some(0.75);
        session.group = Some("backend".to_string());
        session.color = Some("#FF5733".to_string());

        let mut state = AppState::default();
        state.sessions.push(session);

        storage.save_state(&state).unwrap();
        let loaded = storage.load_state().unwrap();

        let loaded_session = &loaded.sessions[0];
        assert_eq!(loaded_session.status, SessionStatus::Working);
        assert_eq!(
            loaded_session.current_task,
            Some(TaskId("task-001".to_string()))
        );
        assert_eq!(loaded_session.context_usage, Some(0.75));
        assert_eq!(loaded_session.group, Some("backend".to_string()));
        assert_eq!(loaded_session.color, Some("#FF5733".to_string()));
    }

    // ========== Task CRUD Tests ==========

    #[test]
    fn test_save_and_load_task() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let task = create_test_task("task-001", "Test Task");
        storage.save_task(&task).unwrap();

        let loaded = storage.load_task(&task.id).unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, TaskId("task-001".to_string()));
        assert_eq!(loaded.title, "Test Task");
    }

    #[test]
    fn test_load_task_returns_none_when_missing() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let result = storage
            .load_task(&TaskId("nonexistent".to_string()))
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_save_task_creates_file() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let task = create_test_task("task-001", "Test Task");
        storage.save_task(&task).unwrap();

        let task_file = storage.task_path(&task.id);
        assert!(task_file.exists());
    }

    #[test]
    fn test_save_task_overwrites_existing() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let mut task = create_test_task("task-001", "Original Title");
        storage.save_task(&task).unwrap();

        task.title = "Updated Title".to_string();
        task.status = TaskStatus::Working;
        storage.save_task(&task).unwrap();

        let loaded = storage.load_task(&task.id).unwrap().unwrap();
        assert_eq!(loaded.title, "Updated Title");
        assert_eq!(loaded.status, TaskStatus::Working);
    }

    #[test]
    fn test_task_with_all_fields() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let mut task = create_test_task("task-001", "Full Task");
        task.priority = TaskPriority::Critical;
        task.status = TaskStatus::Working;
        task.dependencies = vec![TaskId("task-000".to_string())];
        task.tags = vec!["backend".to_string(), "urgent".to_string()];
        task.assigned_session = Some(SessionId(1));
        task.assigned_at = Some(chrono::Utc::now());
        task.started_at = Some(chrono::Utc::now());

        storage.save_task(&task).unwrap();
        let loaded = storage.load_task(&task.id).unwrap().unwrap();

        assert_eq!(loaded.priority, TaskPriority::Critical);
        assert_eq!(loaded.status, TaskStatus::Working);
        assert_eq!(loaded.dependencies.len(), 1);
        assert_eq!(loaded.tags.len(), 2);
        assert_eq!(loaded.assigned_session, Some(SessionId(1)));
        assert!(loaded.assigned_at.is_some());
        assert!(loaded.started_at.is_some());
    }

    #[test]
    fn test_list_task_ids_empty() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let ids = storage.list_task_ids().unwrap();
        assert!(ids.is_empty());
    }

    #[test]
    fn test_list_task_ids() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Create some tasks
        for i in 1..=3 {
            let task = create_test_task(&format!("task-{:03}", i), &format!("Task {}", i));
            storage.save_task(&task).unwrap();
        }

        let ids = storage.list_task_ids().unwrap();
        assert_eq!(ids.len(), 3);
        // Should be sorted
        assert_eq!(ids[0].0, "task-001");
        assert_eq!(ids[1].0, "task-002");
        assert_eq!(ids[2].0, "task-003");
    }

    #[test]
    fn test_list_task_ids_sorted() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Create tasks in non-alphabetical order
        storage.save_task(&create_test_task("zzz", "Last")).unwrap();
        storage
            .save_task(&create_test_task("aaa", "First"))
            .unwrap();
        storage
            .save_task(&create_test_task("mmm", "Middle"))
            .unwrap();

        let ids = storage.list_task_ids().unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0].0, "aaa");
        assert_eq!(ids[1].0, "mmm");
        assert_eq!(ids[2].0, "zzz");
    }

    #[test]
    fn test_list_task_ids_ignores_tmp_files() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Create a real task
        storage
            .save_task(&create_test_task("task-001", "Real Task"))
            .unwrap();

        // Create a temp file manually
        let tmp_path = storage.tasks_dir().join("task-tmp.json.tmp");
        fs::write(&tmp_path, "{}").unwrap();

        let ids = storage.list_task_ids().unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].0, "task-001");
    }

    #[test]
    fn test_delete_task() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let task = create_test_task("to-delete", "Delete Me");
        storage.save_task(&task).unwrap();
        assert!(storage.load_task(&task.id).unwrap().is_some());

        storage.delete_task(&task.id).unwrap();
        assert!(storage.load_task(&task.id).unwrap().is_none());
    }

    #[test]
    fn test_delete_task_nonexistent_succeeds() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Deleting a non-existent task should not error
        let result = storage.delete_task(&TaskId("nonexistent".to_string()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_task_file_removed() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let task = create_test_task("task-001", "Test Task");
        storage.save_task(&task).unwrap();

        let task_file = storage.task_path(&task.id);
        assert!(task_file.exists());

        storage.delete_task(&task.id).unwrap();
        assert!(!task_file.exists());
    }

    // ========== Atomic Write Tests ==========

    #[test]
    fn test_atomic_write() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let path = storage.codirigent_dir.join("test.json");
        storage.atomic_write(&path, r#"{"test": true}"#).unwrap();

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, r#"{"test": true}"#);
    }

    #[test]
    fn test_atomic_write_no_tmp_file_left() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let path = storage.codirigent_dir.join("test.json");
        storage.atomic_write(&path, r#"{"test": true}"#).unwrap();

        let tmp_path = path.with_extension("json.tmp");
        assert!(!tmp_path.exists());
    }

    #[test]
    fn test_atomic_write_overwrites() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let path = storage.codirigent_dir.join("test.json");
        storage.atomic_write(&path, r#"{"version": 1}"#).unwrap();
        storage.atomic_write(&path, r#"{"version": 2}"#).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, r#"{"version": 2}"#);
    }

    // ========== Crash Recovery Tests ==========

    #[test]
    fn test_crash_recovery_simulation() {
        let temp = TempDir::new().unwrap();

        // Create storage and save state
        {
            let storage = FileStorageService::new(temp.path()).unwrap();
            let mut state = AppState::default();
            state.sessions.push(Session::new(
                SessionId(1),
                "Persistent".to_string(),
                PathBuf::from("/project"),
            ));
            storage.save_state(&state).unwrap();
        }

        // "Crash" and restart - create new storage instance
        {
            let storage = FileStorageService::new(temp.path()).unwrap();
            let recovered = storage.load_state().unwrap();

            assert_eq!(recovered.sessions.len(), 1);
            assert_eq!(recovered.sessions[0].id, SessionId(1));
            assert_eq!(recovered.sessions[0].name, "Persistent");
        }
    }

    #[test]
    fn test_task_persistence_across_restarts() {
        let temp = TempDir::new().unwrap();

        // Create tasks
        {
            let storage = FileStorageService::new(temp.path()).unwrap();
            for i in 1..=3 {
                let task = create_test_task(&format!("task-{:03}", i), &format!("Task {}", i));
                storage.save_task(&task).unwrap();
            }
        }

        // "Restart"
        {
            let storage = FileStorageService::new(temp.path()).unwrap();
            let ids = storage.list_task_ids().unwrap();
            assert_eq!(ids.len(), 3);

            for i in 1..=3 {
                let task = storage
                    .load_task(&TaskId(format!("task-{:03}", i)))
                    .unwrap();
                assert!(task.is_some());
            }
        }
    }

    // ========== Error Handling Tests ==========

    #[test]
    fn test_load_state_with_corrupted_file() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Write invalid JSON
        fs::write(storage.state_path(), "not valid json").unwrap();

        let result = storage.load_state();
        assert!(result.is_err());
    }

    #[test]
    fn test_load_task_with_corrupted_file() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Write invalid JSON to task file
        let task_id = TaskId("corrupted".to_string());
        fs::write(storage.task_path(&task_id), "not valid json").unwrap();

        let result = storage.load_task(&task_id);
        assert!(result.is_err());
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_empty_task_id() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let task = create_test_task("", "Empty ID Task");
        storage.save_task(&task).unwrap();

        let loaded = storage.load_task(&TaskId("".to_string())).unwrap();
        assert!(loaded.is_some());
    }

    #[test]
    fn test_task_id_with_spaces() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        // Spaces in task IDs are sanitized to underscores, so the same
        // sanitized path is used for save and load
        let task = create_test_task("task with spaces", "Spaced Task");
        storage.save_task(&task).unwrap();

        // The file is saved with sanitized name "task_with_spaces.json"
        let sanitized_path = storage.tasks_dir().join("task_with_spaces.json");
        assert!(sanitized_path.exists());

        // Loading with original ID works because both use same sanitization
        let loaded = storage
            .load_task(&TaskId("task with spaces".to_string()))
            .unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().title, "Spaced Task");
    }

    #[test]
    fn test_unicode_in_task() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let mut task = create_test_task("unicode-task", "Unicode Test");
        task.description = "Description with unicode: ".to_string();
        task.tags = vec!["".to_string(), "".to_string()];

        storage.save_task(&task).unwrap();
        let loaded = storage.load_task(&task.id).unwrap().unwrap();

        assert!(loaded.description.contains(""));
        assert!(loaded.tags.contains(&"".to_string()));
    }

    #[test]
    fn test_large_state() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let mut state = AppState::default();
        for i in 0..100 {
            state.sessions.push(Session::new(
                SessionId(i),
                format!("Session {} with a longer name to increase size", i),
                PathBuf::from(format!("/very/long/path/to/project/{}/workspace", i)),
            ));
        }

        storage.save_state(&state).unwrap();
        let loaded = storage.load_state().unwrap();

        assert_eq!(loaded.sessions.len(), 100);
    }

    #[test]
    fn test_send_sync_traits() {
        // Verify FileStorageService can be sent across threads
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<FileStorageService>();
        assert_sync::<FileStorageService>();
    }

    // ========== Context Directory Tests ==========

    #[test]
    fn test_context_dir_path() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let expected = temp.path().join(".codirigent").join("context");
        assert_eq!(storage.context_dir(), expected);
        assert!(storage.context_dir().exists());
    }

    #[test]
    fn test_context_file_path() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let path = storage.context_file(SessionId(42));
        let expected = temp
            .path()
            .join(".codirigent")
            .join("context")
            .join("session-42.json");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_context_file_data_serialization() {
        let data = super::ContextFileData {
            usage: 0.42,
            tokens_used: Some(84000),
            tokens_total: Some(200000),
        };
        let json = serde_json::to_string(&data).unwrap();
        let parsed: super::ContextFileData = serde_json::from_str(&json).unwrap();

        assert!((parsed.usage - 0.42).abs() < f32::EPSILON);
        assert_eq!(parsed.tokens_used, Some(84000));
        assert_eq!(parsed.tokens_total, Some(200000));
    }

    #[test]
    fn test_context_file_data_minimal() {
        // Only usage field required
        let json = r#"{"usage": 0.35}"#;
        let parsed: super::ContextFileData = serde_json::from_str(json).unwrap();

        assert!((parsed.usage - 0.35).abs() < f32::EPSILON);
        assert!(parsed.tokens_used.is_none());
        assert!(parsed.tokens_total.is_none());
    }

    #[test]
    fn test_context_file_data_skips_none_fields() {
        let data = super::ContextFileData {
            usage: 0.5,
            tokens_used: None,
            tokens_total: None,
        };
        let json = serde_json::to_string(&data).unwrap();

        // Optional fields should be omitted
        assert!(!json.contains("tokens_used"));
        assert!(!json.contains("tokens_total"));
    }

    #[test]
    fn test_context_file_roundtrip() {
        let temp = TempDir::new().unwrap();
        let storage = FileStorageService::new(temp.path()).unwrap();

        let data = super::ContextFileData {
            usage: 0.73,
            tokens_used: Some(146000),
            tokens_total: Some(200000),
        };

        let path = storage.context_file(SessionId(1));
        let content = serde_json::to_string(&data).unwrap();
        fs::write(&path, &content).unwrap();

        let read_content = fs::read_to_string(&path).unwrap();
        let parsed: super::ContextFileData = serde_json::from_str(&read_content).unwrap();
        assert!((parsed.usage - 0.73).abs() < f32::EPSILON);
    }
}
