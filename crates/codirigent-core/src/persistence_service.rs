//! Persistence service trait and implementations.
//!
//! This module provides the [`PersistenceService`] trait for session
//! persistence and recovery, along with configuration types.
//!
//! ## Components
//!
//! - [`PersistenceService`]: Trait for persistence operations
//! - [`DefaultPersistenceService`]: File-based implementation
//! - [`AutoSaveConfig`]: Configuration for automatic saving

use crate::persistence::{Checkpoint, PersistentState, RecoveryResult};
use crate::types::SessionId;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Service for session persistence and recovery.
///
/// Provides methods for saving and loading application state,
/// creating checkpoints, and recovering sessions.
///
/// # Implementors
///
/// - [`DefaultPersistenceService`]: File-based persistence using JSON files
pub trait PersistenceService: Send + Sync {
    /// Save current application state.
    ///
    /// The state should be written atomically to prevent corruption.
    ///
    /// # Arguments
    ///
    /// * `state` - The state to save
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    fn save_state(&self, state: &PersistentState) -> Result<()>;

    /// Load application state from disk.
    ///
    /// # Returns
    ///
    /// The loaded state, or `None` if no state file exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the state file exists but cannot be read.
    fn load_state(&self) -> Result<Option<PersistentState>>;

    /// Create a named checkpoint.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the checkpoint
    /// * `state` - The state to checkpoint
    ///
    /// # Returns
    ///
    /// The created checkpoint with its generated ID.
    ///
    /// # Errors
    ///
    /// Returns an error if checkpoint creation fails.
    fn create_checkpoint(&self, name: &str, state: &PersistentState) -> Result<Checkpoint>;

    /// List all checkpoints.
    ///
    /// # Returns
    ///
    /// A list of checkpoints, sorted by creation time (newest first).
    ///
    /// # Errors
    ///
    /// Returns an error if reading checkpoints fails.
    fn list_checkpoints(&self) -> Result<Vec<Checkpoint>>;

    /// Load a specific checkpoint.
    ///
    /// # Arguments
    ///
    /// * `id` - The checkpoint ID to load
    ///
    /// # Returns
    ///
    /// The checkpoint if found, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint exists but cannot be read.
    fn load_checkpoint(&self, id: &str) -> Result<Option<Checkpoint>>;

    /// Delete a checkpoint.
    ///
    /// # Arguments
    ///
    /// * `id` - The checkpoint ID to delete
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    fn delete_checkpoint(&self, id: &str) -> Result<()>;

    /// Attempt to recover a session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID to recover
    ///
    /// # Returns
    ///
    /// The recovery result indicating success, partial success, or failure.
    ///
    /// # Errors
    ///
    /// Returns an error if recovery cannot be attempted.
    fn recover_session(&self, session_id: SessionId) -> Result<RecoveryResult>;

    /// Check if recovery data exists.
    ///
    /// # Returns
    ///
    /// `true` if a state file exists that can be used for recovery.
    fn has_recovery_data(&self) -> bool;

    /// Clear all recovery data.
    ///
    /// This removes the state file but not checkpoints.
    ///
    /// # Errors
    ///
    /// Returns an error if clearing fails.
    fn clear_recovery_data(&self) -> Result<()>;

    /// Get the state file path.
    ///
    /// # Returns
    ///
    /// The path to the state.json file.
    fn state_file_path(&self) -> PathBuf;

    /// Get the checkpoints directory path.
    ///
    /// # Returns
    ///
    /// The path to the checkpoints directory.
    fn checkpoints_dir(&self) -> PathBuf;
}

/// Auto-save configuration.
///
/// Controls automatic state saving behavior.
///
/// # Example
///
/// ```
/// use codirigent_core::persistence_service::AutoSaveConfig;
///
/// let config = AutoSaveConfig::default();
/// assert!(config.enabled);
/// assert_eq!(config.interval_seconds, 30);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoSaveConfig {
    /// Enable auto-save.
    pub enabled: bool,
    /// Interval in seconds between saves.
    pub interval_seconds: u32,
    /// Save on session status change.
    pub on_status_change: bool,
    /// Save before close.
    pub on_close: bool,
    /// Save scrollback buffer (may increase file size).
    pub save_scrollback: bool,
}

impl Default for AutoSaveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_seconds: 30,
            on_status_change: true,
            on_close: true,
            save_scrollback: false,
        }
    }
}

impl AutoSaveConfig {
    /// Create a new auto-save config with custom interval.
    ///
    /// # Arguments
    ///
    /// * `interval_seconds` - Interval between automatic saves
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence_service::AutoSaveConfig;
    ///
    /// let config = AutoSaveConfig::with_interval(60);
    /// assert_eq!(config.interval_seconds, 60);
    /// ```
    pub fn with_interval(interval_seconds: u32) -> Self {
        Self {
            interval_seconds,
            ..Default::default()
        }
    }

    /// Create a disabled auto-save config.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence_service::AutoSaveConfig;
    ///
    /// let config = AutoSaveConfig::disabled();
    /// assert!(!config.enabled);
    /// ```
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }
}

/// Default implementation of PersistenceService.
///
/// Uses the `.codirigent` directory in the project for storage.
/// State is stored in `state.json` and checkpoints in `checkpoints/`.
///
/// # File Structure
///
/// ```text
/// .codirigent/
/// +-- state.json           # Current application state
/// +-- checkpoints/         # Named checkpoints
///     +-- checkpoint-xxx.json
/// ```
///
/// # Example
///
/// ```no_run
/// use codirigent_core::persistence_service::{DefaultPersistenceService, PersistenceService};
/// use codirigent_core::persistence::PersistentState;
/// use std::path::Path;
///
/// let service = DefaultPersistenceService::new(Path::new("/project"));
/// let state = PersistentState::new();
/// service.save_state(&state).unwrap();
/// ```
pub struct DefaultPersistenceService {
    codirigent_dir: PathBuf,
}

impl DefaultPersistenceService {
    /// Create a new persistence service.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - The project root directory
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence_service::DefaultPersistenceService;
    /// use std::path::Path;
    ///
    /// let service = DefaultPersistenceService::new(Path::new("/project"));
    /// ```
    pub fn new(project_dir: &Path) -> Self {
        Self {
            codirigent_dir: project_dir.join(".codirigent"),
        }
    }

    /// Get the state file path.
    fn state_path(&self) -> PathBuf {
        self.codirigent_dir.join("state.json")
    }

    /// Get the checkpoints directory path.
    fn checkpoints_path(&self) -> PathBuf {
        self.codirigent_dir.join("checkpoints")
    }

    /// Get the path for a specific checkpoint.
    fn checkpoint_path(&self, id: &str) -> PathBuf {
        self.checkpoints_path().join(format!("{}.json", id))
    }

    /// Ensure the necessary directories exist.
    fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.codirigent_dir)?;
        fs::create_dir_all(self.checkpoints_path())?;
        Ok(())
    }

    /// Get the dirigent directory path.
    pub fn codirigent_dir(&self) -> &Path {
        &self.codirigent_dir
    }
}

impl PersistenceService for DefaultPersistenceService {
    fn save_state(&self, state: &PersistentState) -> Result<()> {
        self.ensure_dirs()?;

        let json = serde_json::to_string_pretty(state)?;
        let temp_path = self.state_path().with_extension("tmp");

        // Write to temp file first, then rename for atomicity
        fs::write(&temp_path, &json)?;
        fs::rename(&temp_path, self.state_path())?;

        Ok(())
    }

    fn load_state(&self) -> Result<Option<PersistentState>> {
        let path = self.state_path();
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;
        let state: PersistentState = serde_json::from_str(&content)?;
        Ok(Some(state))
    }

    fn create_checkpoint(&self, name: &str, state: &PersistentState) -> Result<Checkpoint> {
        self.ensure_dirs()?;

        let checkpoint = Checkpoint::new(name.to_string(), state.clone());
        let json = serde_json::to_string_pretty(&checkpoint)?;
        fs::write(self.checkpoint_path(&checkpoint.id), json)?;

        Ok(checkpoint)
    }

    fn list_checkpoints(&self) -> Result<Vec<Checkpoint>> {
        let dir = self.checkpoints_path();
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut checkpoints = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let content = fs::read_to_string(&path)?;
                let checkpoint: Checkpoint = serde_json::from_str(&content)?;
                checkpoints.push(checkpoint);
            }
        }

        // Sort by creation time, newest first
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(checkpoints)
    }

    fn load_checkpoint(&self, id: &str) -> Result<Option<Checkpoint>> {
        let path = self.checkpoint_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;
        let checkpoint: Checkpoint = serde_json::from_str(&content)?;
        Ok(Some(checkpoint))
    }

    fn delete_checkpoint(&self, id: &str) -> Result<()> {
        let path = self.checkpoint_path(id);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn recover_session(&self, session_id: SessionId) -> Result<RecoveryResult> {
        let state = self
            .load_state()?
            .ok_or_else(|| anyhow::anyhow!("No recovery data found"))?;

        let session = state
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found in recovery data"))?;

        // Check if working directory still exists
        if !session.working_directory.exists() {
            return Ok(RecoveryResult::Failed(format!(
                "Working directory no longer exists: {:?}",
                session.working_directory
            )));
        }

        // Check for partial restore conditions
        let mut warnings = Vec::new();

        if session.scrollback_hash.is_some() {
            warnings.push("Scrollback buffer not restored".to_string());
        }

        if session.context_usage.is_some() {
            warnings.push("Context usage reset to zero".to_string());
        }

        if let Some(worktree) = &session.worktree_path {
            if !worktree.exists() {
                warnings.push(format!("Worktree directory missing: {:?}", worktree));
            }
        }

        if warnings.is_empty() {
            Ok(RecoveryResult::Restored(session_id))
        } else {
            Ok(RecoveryResult::PartialRestore(session_id, warnings))
        }
    }

    fn has_recovery_data(&self) -> bool {
        self.state_path().exists()
    }

    fn clear_recovery_data(&self) -> Result<()> {
        let path = self.state_path();
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn state_file_path(&self) -> PathBuf {
        self.state_path()
    }

    fn checkpoints_dir(&self) -> PathBuf {
        self.checkpoints_path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::PersistentSession;
    use crate::types::Session;
    use tempfile::tempdir;

    // AutoSaveConfig tests

    #[test]
    fn test_auto_save_config_default() {
        let config = AutoSaveConfig::default();
        assert!(config.enabled);
        assert_eq!(config.interval_seconds, 30);
        assert!(config.on_status_change);
        assert!(config.on_close);
        assert!(!config.save_scrollback);
    }

    #[test]
    fn test_auto_save_config_with_interval() {
        let config = AutoSaveConfig::with_interval(60);
        assert!(config.enabled);
        assert_eq!(config.interval_seconds, 60);
    }

    #[test]
    fn test_auto_save_config_disabled() {
        let config = AutoSaveConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_auto_save_config_serialization() {
        let config = AutoSaveConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AutoSaveConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn test_auto_save_config_equality() {
        let config1 = AutoSaveConfig::default();
        let config2 = AutoSaveConfig::default();
        let config3 = AutoSaveConfig::with_interval(60);
        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    // DefaultPersistenceService tests

    #[test]
    fn test_persistence_service_new() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());
        assert_eq!(
            service.codirigent_dir(),
            temp.path().join(".codirigent").as_path()
        );
    }

    #[test]
    fn test_save_and_load_state() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let mut state = PersistentState::new();
        let session = Session::new(SessionId(1), "Test".to_string(), temp.path().to_path_buf());
        state.add_session(PersistentSession::from_session(&session));

        service.save_state(&state).unwrap();

        let loaded = service.load_state().unwrap().unwrap();
        assert_eq!(loaded.sessions.len(), 1);
        assert_eq!(loaded.sessions[0].name, "Test");
    }

    #[test]
    fn test_load_state_no_file() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let state = service.load_state().unwrap();
        assert!(state.is_none());
    }

    #[test]
    fn test_has_recovery_data() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        assert!(!service.has_recovery_data());

        let state = PersistentState::new();
        service.save_state(&state).unwrap();

        assert!(service.has_recovery_data());
    }

    #[test]
    fn test_clear_recovery_data() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let state = PersistentState::new();
        service.save_state(&state).unwrap();
        assert!(service.has_recovery_data());

        service.clear_recovery_data().unwrap();
        assert!(!service.has_recovery_data());
    }

    #[test]
    fn test_clear_recovery_data_no_file() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        // Should not error if file doesn't exist
        service.clear_recovery_data().unwrap();
    }

    #[test]
    fn test_checkpoint_lifecycle() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());
        let state = PersistentState::new();

        // Create checkpoint
        let checkpoint = service
            .create_checkpoint("Before refactor", &state)
            .unwrap();
        assert_eq!(checkpoint.name, "Before refactor");
        assert!(checkpoint.id.starts_with("checkpoint-"));

        // List checkpoints
        let checkpoints = service.list_checkpoints().unwrap();
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].id, checkpoint.id);

        // Load checkpoint
        let loaded = service.load_checkpoint(&checkpoint.id).unwrap().unwrap();
        assert_eq!(loaded.name, checkpoint.name);

        // Delete checkpoint
        service.delete_checkpoint(&checkpoint.id).unwrap();
        let checkpoints = service.list_checkpoints().unwrap();
        assert!(checkpoints.is_empty());
    }

    #[test]
    fn test_load_nonexistent_checkpoint() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let checkpoint = service.load_checkpoint("nonexistent").unwrap();
        assert!(checkpoint.is_none());
    }

    #[test]
    fn test_delete_nonexistent_checkpoint() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        // Should not error
        service.delete_checkpoint("nonexistent").unwrap();
    }

    #[test]
    fn test_list_checkpoints_empty() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let checkpoints = service.list_checkpoints().unwrap();
        assert!(checkpoints.is_empty());
    }

    #[test]
    fn test_list_checkpoints_sorted() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());
        let state = PersistentState::new();

        // Create multiple checkpoints
        service.create_checkpoint("First", &state).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        service.create_checkpoint("Second", &state).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        service.create_checkpoint("Third", &state).unwrap();

        let checkpoints = service.list_checkpoints().unwrap();
        assert_eq!(checkpoints.len(), 3);
        // Should be sorted newest first
        assert_eq!(checkpoints[0].name, "Third");
        assert_eq!(checkpoints[1].name, "Second");
        assert_eq!(checkpoints[2].name, "First");
    }

    #[test]
    fn test_recover_session_success() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let mut state = PersistentState::new();
        // Use temp directory as working directory so it exists
        let session = Session::new(SessionId(1), "Test".to_string(), temp.path().to_path_buf());
        state.add_session(PersistentSession::from_session(&session));
        service.save_state(&state).unwrap();

        let result = service.recover_session(SessionId(1)).unwrap();
        assert!(result.is_success());
        assert_eq!(result.session_id(), Some(SessionId(1)));
    }

    #[test]
    fn test_recover_session_with_warnings() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let mut state = PersistentState::new();
        let session = Session::new(SessionId(1), "Test".to_string(), temp.path().to_path_buf());
        let mut persistent = PersistentSession::from_session(&session);
        persistent.context_usage = Some(0.5); // Will trigger warning
        state.add_session(persistent);
        service.save_state(&state).unwrap();

        let result = service.recover_session(SessionId(1)).unwrap();
        assert!(result.is_success());
        assert!(!result.warnings().is_empty());
    }

    #[test]
    fn test_recover_session_missing_directory() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let mut state = PersistentState::new();
        // Use a non-existent directory
        let session = Session::new(
            SessionId(1),
            "Test".to_string(),
            PathBuf::from("/nonexistent/directory"),
        );
        state.add_session(PersistentSession::from_session(&session));
        service.save_state(&state).unwrap();

        let result = service.recover_session(SessionId(1)).unwrap();
        assert!(!result.is_success());
        assert!(result.error().is_some());
    }

    #[test]
    fn test_recover_session_no_recovery_data() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let result = service.recover_session(SessionId(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_recover_session_not_found() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let state = PersistentState::new();
        service.save_state(&state).unwrap();

        let result = service.recover_session(SessionId(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_state_file_path() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let path = service.state_file_path();
        assert_eq!(path, temp.path().join(".codirigent").join("state.json"));
    }

    #[test]
    fn test_checkpoints_dir() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let path = service.checkpoints_dir();
        assert_eq!(path, temp.path().join(".codirigent").join("checkpoints"));
    }

    #[test]
    fn test_atomic_save() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        // Save state
        let state = PersistentState::new();
        service.save_state(&state).unwrap();

        // Verify no temp file remains
        let temp_path = service.state_file_path().with_extension("tmp");
        assert!(!temp_path.exists());

        // Verify state file exists
        assert!(service.state_file_path().exists());
    }

    #[test]
    fn test_persistence_service_trait_object_safe() {
        // This compiles only if PersistenceService is object-safe
        fn _takes_persistence_service(_: &dyn PersistenceService) {}
    }

    #[test]
    fn test_recover_session_with_missing_worktree() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let mut state = PersistentState::new();
        let session = Session::new(SessionId(1), "Test".to_string(), temp.path().to_path_buf());
        let mut persistent = PersistentSession::from_session(&session);
        persistent.worktree_path = Some(PathBuf::from("/nonexistent/worktree"));
        state.add_session(persistent);
        service.save_state(&state).unwrap();

        let result = service.recover_session(SessionId(1)).unwrap();
        assert!(result.is_success()); // Partial success
        assert!(!result.warnings().is_empty());
        assert!(result.warnings().iter().any(|w| w.contains("Worktree")));
    }

    #[test]
    fn test_recover_session_with_scrollback() {
        let temp = tempdir().unwrap();
        let service = DefaultPersistenceService::new(temp.path());

        let mut state = PersistentState::new();
        let session = Session::new(SessionId(1), "Test".to_string(), temp.path().to_path_buf());
        let persistent =
            PersistentSession::from_session(&session).with_scrollback_hash("abc123".to_string());
        state.add_session(persistent);
        service.save_state(&state).unwrap();

        let result = service.recover_session(SessionId(1)).unwrap();
        assert!(result.is_success());
        assert!(!result.warnings().is_empty());
        assert!(result.warnings().iter().any(|w| w.contains("Scrollback")));
    }
}
