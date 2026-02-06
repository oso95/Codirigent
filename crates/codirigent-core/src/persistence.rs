//! Session persistence types for saving and restoring session state.
//!
//! This module provides types for persisting session state to disk,
//! enabling session recovery after application restart or crash.
//!
//! ## Types
//!
//! - [`PersistentSession`]: Persistent representation of a session
//! - [`PersistentState`]: Application state snapshot for persistence
//! - [`Checkpoint`]: Named snapshot for manual save points
//! - [`RecoveryResult`]: Result of session recovery attempt

use crate::types::{LayoutMode, Session, SessionId, SessionStatus, TaskId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persistent session state saved to disk.
///
/// Contains all information needed to restore a session after
/// application restart. Some fields like `context_usage` are
/// reset on restore since they represent runtime state.
///
/// # Example
///
/// ```
/// use codirigent_core::persistence::PersistentSession;
/// use codirigent_core::{Session, SessionId, SessionStatus};
/// use std::path::PathBuf;
///
/// let session = Session::new(
///     SessionId(1),
///     "Test".to_string(),
///     PathBuf::from("/tmp"),
/// );
/// let persistent = PersistentSession::from_session(&session);
/// assert_eq!(persistent.id, session.id);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistentSession {
    /// Session identifier.
    pub id: SessionId,
    /// Session name.
    pub name: String,
    /// Last known status.
    pub status: SessionStatus,
    /// Working directory.
    pub working_directory: PathBuf,
    /// Current task if any.
    pub current_task: Option<TaskId>,
    /// Git worktree path if using worktrees.
    pub worktree_path: Option<PathBuf>,
    /// Last known context usage percentage.
    pub context_usage: Option<f32>,
    /// When the session was started.
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// Last checkpoint time.
    pub last_checkpoint: chrono::DateTime<chrono::Utc>,
    /// Terminal scrollback buffer hash (for verification on restore).
    pub scrollback_hash: Option<String>,
    /// Session group.
    pub group: Option<String>,
    /// Session color.
    pub color: Option<String>,
}

impl PersistentSession {
    /// Create a persistent session from a regular Session.
    ///
    /// # Arguments
    ///
    /// * `session` - The session to convert
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence::PersistentSession;
    /// use codirigent_core::{Session, SessionId};
    /// use std::path::PathBuf;
    ///
    /// let session = Session::new(
    ///     SessionId(1),
    ///     "Test".to_string(),
    ///     PathBuf::from("/tmp"),
    /// );
    /// let persistent = PersistentSession::from_session(&session);
    /// assert_eq!(persistent.name, "Test");
    /// ```
    pub fn from_session(session: &Session) -> Self {
        Self {
            id: session.id,
            name: session.name.clone(),
            status: session.status,
            working_directory: session.working_directory.clone(),
            current_task: session.current_task.clone(),
            worktree_path: None,
            context_usage: session.context_usage,
            started_at: session.created_at,
            last_checkpoint: chrono::Utc::now(),
            scrollback_hash: None,
            group: session.group.clone(),
            color: session.color.clone(),
        }
    }

    /// Create a persistent session with worktree path.
    ///
    /// # Arguments
    ///
    /// * `session` - The session to convert
    /// * `worktree_path` - Path to the git worktree
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence::PersistentSession;
    /// use codirigent_core::{Session, SessionId};
    /// use std::path::PathBuf;
    ///
    /// let session = Session::new(
    ///     SessionId(1),
    ///     "Test".to_string(),
    ///     PathBuf::from("/tmp"),
    /// );
    /// let persistent = PersistentSession::from_session_with_worktree(
    ///     &session,
    ///     PathBuf::from("/repo/worktrees/feature"),
    /// );
    /// assert!(persistent.worktree_path.is_some());
    /// ```
    pub fn from_session_with_worktree(session: &Session, worktree_path: PathBuf) -> Self {
        let mut persistent = Self::from_session(session);
        persistent.worktree_path = Some(worktree_path);
        persistent
    }

    /// Convert back to a Session (for restoration).
    ///
    /// Note: Status is reset to Idle and context_usage is reset to None
    /// since these represent runtime state that cannot be restored.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence::PersistentSession;
    /// use codirigent_core::{Session, SessionId, SessionStatus};
    /// use std::path::PathBuf;
    ///
    /// let session = Session::new(
    ///     SessionId(1),
    ///     "Test".to_string(),
    ///     PathBuf::from("/tmp"),
    /// );
    /// let persistent = PersistentSession::from_session(&session);
    /// let restored = persistent.to_session();
    /// assert_eq!(restored.status, SessionStatus::Idle);
    /// ```
    pub fn to_session(&self) -> Session {
        Session {
            id: self.id,
            name: self.name.clone(),
            status: SessionStatus::Idle, // Reset status on restore
            working_directory: self.working_directory.clone(),
            current_task: self.current_task.clone(),
            context_usage: None, // Reset on restore
            created_at: self.started_at,
            group: self.group.clone(),
            color: self.color.clone(),
            git_info: None, // Re-detected on restore
        }
    }

    /// Set the scrollback hash for this session.
    ///
    /// # Arguments
    ///
    /// * `hash` - The hash of the scrollback buffer content
    pub fn with_scrollback_hash(mut self, hash: String) -> Self {
        self.scrollback_hash = Some(hash);
        self
    }

    /// Update the last checkpoint time to now.
    pub fn update_checkpoint_time(&mut self) {
        self.last_checkpoint = chrono::Utc::now();
    }
}

/// Application state for persistence.
///
/// Contains all session states, layout configuration, and metadata
/// needed to restore the application state after restart.
///
/// # Example
///
/// ```
/// use codirigent_core::persistence::PersistentState;
///
/// let state = PersistentState::new();
/// assert!(state.sessions.is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistentState {
    /// All session states.
    pub sessions: Vec<PersistentSession>,
    /// Active session ID.
    pub active_session: Option<SessionId>,
    /// Layout configuration.
    pub layout: LayoutMode,
    /// Last update timestamp.
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Application version for migration.
    pub app_version: String,
}

impl Default for PersistentState {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistentState {
    /// Create a new empty persistent state with current timestamp.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence::PersistentState;
    ///
    /// let state = PersistentState::new();
    /// assert!(state.sessions.is_empty());
    /// assert!(state.active_session.is_none());
    /// ```
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            active_session: None,
            layout: LayoutMode::default(),
            updated_at: chrono::Utc::now(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Add a session to the persistent state.
    ///
    /// # Arguments
    ///
    /// * `session` - The session to add
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence::{PersistentSession, PersistentState};
    /// use codirigent_core::{Session, SessionId};
    /// use std::path::PathBuf;
    ///
    /// let mut state = PersistentState::new();
    /// let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
    /// state.add_session(PersistentSession::from_session(&session));
    /// assert_eq!(state.sessions.len(), 1);
    /// ```
    pub fn add_session(&mut self, session: PersistentSession) {
        self.sessions.push(session);
        self.updated_at = chrono::Utc::now();
    }

    /// Remove a session from the persistent state.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID to remove
    ///
    /// # Returns
    ///
    /// `true` if the session was found and removed, `false` otherwise.
    pub fn remove_session(&mut self, id: SessionId) -> bool {
        let initial_len = self.sessions.len();
        self.sessions.retain(|s| s.id != id);
        let removed = self.sessions.len() < initial_len;
        if removed {
            self.updated_at = chrono::Utc::now();
            if self.active_session == Some(id) {
                self.active_session = None;
            }
        }
        removed
    }

    /// Get a session by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID to find
    pub fn get_session(&self, id: SessionId) -> Option<&PersistentSession> {
        self.sessions.iter().find(|s| s.id == id)
    }

    /// Get a mutable reference to a session by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID to find
    pub fn get_session_mut(&mut self, id: SessionId) -> Option<&mut PersistentSession> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    /// Update the timestamp to now.
    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now();
    }
}

/// Checkpoint metadata for named save points.
///
/// Checkpoints allow users to create named snapshots of the
/// application state that can be restored later.
///
/// # Example
///
/// ```
/// use codirigent_core::persistence::{Checkpoint, PersistentState};
///
/// let state = PersistentState::new();
/// let checkpoint = Checkpoint::new("Before refactor".to_string(), state);
/// assert_eq!(checkpoint.name, "Before refactor");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Checkpoint {
    /// Unique checkpoint ID.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// When created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// State snapshot.
    pub state: PersistentState,
}

impl Checkpoint {
    /// Create a new checkpoint with a generated ID.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the checkpoint
    /// * `state` - The state to snapshot
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence::{Checkpoint, PersistentState};
    ///
    /// let state = PersistentState::new();
    /// let checkpoint = Checkpoint::new("My checkpoint".to_string(), state);
    /// assert!(!checkpoint.id.is_empty());
    /// ```
    pub fn new(name: String, state: PersistentState) -> Self {
        Self {
            id: Self::generate_id(),
            name,
            created_at: chrono::Utc::now(),
            state,
        }
    }

    /// Create a checkpoint with a specific ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The checkpoint ID
    /// * `name` - Human-readable name
    /// * `state` - The state to snapshot
    pub fn with_id(id: String, name: String, state: PersistentState) -> Self {
        Self {
            id,
            name,
            created_at: chrono::Utc::now(),
            state,
        }
    }

    /// Generate a unique checkpoint ID based on timestamp.
    fn generate_id() -> String {
        let now = chrono::Utc::now();
        format!("checkpoint-{}", now.format("%Y%m%d-%H%M%S-%3f"))
    }
}

/// Session recovery result.
///
/// Indicates the outcome of attempting to recover a session
/// from persistent state.
///
/// # Variants
///
/// - `Restored`: Session was fully restored
/// - `PartialRestore`: Session was restored but some data was lost
/// - `Failed`: Session could not be restored
///
/// # Example
///
/// ```
/// use codirigent_core::persistence::RecoveryResult;
/// use codirigent_core::SessionId;
///
/// let result = RecoveryResult::Restored(SessionId(1));
/// assert!(result.is_success());
/// ```
#[derive(Debug, Clone)]
pub enum RecoveryResult {
    /// Session fully restored.
    Restored(SessionId),
    /// Session partially restored (some data lost).
    PartialRestore(SessionId, Vec<String>),
    /// Session could not be restored.
    Failed(String),
}

impl RecoveryResult {
    /// Check if the recovery was successful (fully or partially).
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence::RecoveryResult;
    /// use codirigent_core::SessionId;
    ///
    /// let full = RecoveryResult::Restored(SessionId(1));
    /// let partial = RecoveryResult::PartialRestore(SessionId(1), vec!["warning".to_string()]);
    /// let failed = RecoveryResult::Failed("error".to_string());
    ///
    /// assert!(full.is_success());
    /// assert!(partial.is_success());
    /// assert!(!failed.is_success());
    /// ```
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            RecoveryResult::Restored(_) | RecoveryResult::PartialRestore(_, _)
        )
    }

    /// Get the session ID if recovery was successful.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence::RecoveryResult;
    /// use codirigent_core::SessionId;
    ///
    /// let result = RecoveryResult::Restored(SessionId(42));
    /// assert_eq!(result.session_id(), Some(SessionId(42)));
    ///
    /// let failed = RecoveryResult::Failed("error".to_string());
    /// assert!(failed.session_id().is_none());
    /// ```
    pub fn session_id(&self) -> Option<SessionId> {
        match self {
            RecoveryResult::Restored(id) | RecoveryResult::PartialRestore(id, _) => Some(*id),
            RecoveryResult::Failed(_) => None,
        }
    }

    /// Get warnings from a partial restore.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence::RecoveryResult;
    /// use codirigent_core::SessionId;
    ///
    /// let partial = RecoveryResult::PartialRestore(
    ///     SessionId(1),
    ///     vec!["Scrollback not restored".to_string()],
    /// );
    /// assert_eq!(partial.warnings().len(), 1);
    ///
    /// let full = RecoveryResult::Restored(SessionId(1));
    /// assert!(full.warnings().is_empty());
    /// ```
    pub fn warnings(&self) -> &[String] {
        match self {
            RecoveryResult::PartialRestore(_, warnings) => warnings,
            _ => &[],
        }
    }

    /// Get the error message if recovery failed.
    ///
    /// # Example
    ///
    /// ```
    /// use codirigent_core::persistence::RecoveryResult;
    /// use codirigent_core::SessionId;
    ///
    /// let failed = RecoveryResult::Failed("Directory not found".to_string());
    /// assert_eq!(failed.error(), Some("Directory not found"));
    ///
    /// let success = RecoveryResult::Restored(SessionId(1));
    /// assert!(success.error().is_none());
    /// ```
    pub fn error(&self) -> Option<&str> {
        match self {
            RecoveryResult::Failed(msg) => Some(msg),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // PersistentSession tests

    #[test]
    fn test_persistent_session_from_session() {
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        let persistent = PersistentSession::from_session(&session);

        assert_eq!(persistent.id, session.id);
        assert_eq!(persistent.name, session.name);
        assert_eq!(persistent.status, session.status);
        assert_eq!(persistent.working_directory, session.working_directory);
        assert!(persistent.worktree_path.is_none());
        assert!(persistent.scrollback_hash.is_none());
    }

    #[test]
    fn test_persistent_session_from_session_with_worktree() {
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        let worktree_path = PathBuf::from("/repo/worktrees/feature");
        let persistent =
            PersistentSession::from_session_with_worktree(&session, worktree_path.clone());

        assert_eq!(persistent.worktree_path, Some(worktree_path));
    }

    #[test]
    fn test_persistent_session_roundtrip() {
        let mut session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        session.group = Some("backend".to_string());
        session.color = Some("#FF0000".to_string());

        let persistent = PersistentSession::from_session(&session);
        let restored = persistent.to_session();

        assert_eq!(restored.id, session.id);
        assert_eq!(restored.name, session.name);
        assert_eq!(restored.working_directory, session.working_directory);
        assert_eq!(restored.group, session.group);
        assert_eq!(restored.color, session.color);
        // Status should be reset to Idle
        assert_eq!(restored.status, SessionStatus::Idle);
        // Context usage should be reset
        assert!(restored.context_usage.is_none());
    }

    #[test]
    fn test_persistent_session_with_scrollback_hash() {
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        let persistent = PersistentSession::from_session(&session)
            .with_scrollback_hash("abc123".to_string());

        assert_eq!(persistent.scrollback_hash, Some("abc123".to_string()));
    }

    #[test]
    fn test_persistent_session_update_checkpoint_time() {
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        let mut persistent = PersistentSession::from_session(&session);

        let before = persistent.last_checkpoint;
        std::thread::sleep(std::time::Duration::from_millis(10));
        persistent.update_checkpoint_time();
        let after = persistent.last_checkpoint;

        assert!(after > before);
    }

    #[test]
    fn test_persistent_session_serialization() {
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        let persistent = PersistentSession::from_session(&session);

        let json = serde_json::to_string(&persistent).unwrap();
        let parsed: PersistentSession = serde_json::from_str(&json).unwrap();

        assert_eq!(persistent.id, parsed.id);
        assert_eq!(persistent.name, parsed.name);
    }

    #[test]
    fn test_persistent_session_equality() {
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        let persistent1 = PersistentSession::from_session(&session);

        // Serialize and deserialize to get an equal instance
        let json = serde_json::to_string(&persistent1).unwrap();
        let persistent2: PersistentSession = serde_json::from_str(&json).unwrap();

        assert_eq!(persistent1, persistent2);
    }

    // PersistentState tests

    #[test]
    fn test_persistent_state_new() {
        let state = PersistentState::new();

        assert!(state.sessions.is_empty());
        assert!(state.active_session.is_none());
        assert!(matches!(
            state.layout,
            LayoutMode::Grid { rows: 2, cols: 2 }
        ));
        assert!(!state.app_version.is_empty());
    }

    #[test]
    fn test_persistent_state_default() {
        let state = PersistentState::default();
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn test_persistent_state_add_session() {
        let mut state = PersistentState::new();
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));

        state.add_session(PersistentSession::from_session(&session));

        assert_eq!(state.sessions.len(), 1);
        assert_eq!(state.sessions[0].id, SessionId(1));
    }

    #[test]
    fn test_persistent_state_remove_session() {
        let mut state = PersistentState::new();
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        state.add_session(PersistentSession::from_session(&session));
        state.active_session = Some(SessionId(1));

        let removed = state.remove_session(SessionId(1));

        assert!(removed);
        assert!(state.sessions.is_empty());
        assert!(state.active_session.is_none());
    }

    #[test]
    fn test_persistent_state_remove_nonexistent_session() {
        let mut state = PersistentState::new();
        let removed = state.remove_session(SessionId(999));
        assert!(!removed);
    }

    #[test]
    fn test_persistent_state_get_session() {
        let mut state = PersistentState::new();
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        state.add_session(PersistentSession::from_session(&session));

        assert!(state.get_session(SessionId(1)).is_some());
        assert!(state.get_session(SessionId(999)).is_none());
    }

    #[test]
    fn test_persistent_state_get_session_mut() {
        let mut state = PersistentState::new();
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        state.add_session(PersistentSession::from_session(&session));

        if let Some(s) = state.get_session_mut(SessionId(1)) {
            s.name = "Modified".to_string();
        }

        assert_eq!(state.sessions[0].name, "Modified");
    }

    #[test]
    fn test_persistent_state_touch() {
        let mut state = PersistentState::new();
        let before = state.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(10));
        state.touch();

        assert!(state.updated_at > before);
    }

    #[test]
    fn test_persistent_state_serialization() {
        let mut state = PersistentState::new();
        let session = Session::new(SessionId(1), "Test".to_string(), PathBuf::from("/tmp"));
        state.add_session(PersistentSession::from_session(&session));
        state.active_session = Some(SessionId(1));
        state.layout = LayoutMode::Grid { rows: 3, cols: 3 };

        let json = serde_json::to_string_pretty(&state).unwrap();
        let parsed: PersistentState = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.sessions.len(), 1);
        assert_eq!(parsed.active_session, Some(SessionId(1)));
        assert!(matches!(
            parsed.layout,
            LayoutMode::Grid { rows: 3, cols: 3 }
        ));
    }

    // Checkpoint tests

    #[test]
    fn test_checkpoint_new() {
        let state = PersistentState::new();
        let checkpoint = Checkpoint::new("Before refactor".to_string(), state);

        assert!(!checkpoint.id.is_empty());
        assert!(checkpoint.id.starts_with("checkpoint-"));
        assert_eq!(checkpoint.name, "Before refactor");
    }

    #[test]
    fn test_checkpoint_with_id() {
        let state = PersistentState::new();
        let checkpoint = Checkpoint::with_id(
            "custom-id".to_string(),
            "Custom checkpoint".to_string(),
            state,
        );

        assert_eq!(checkpoint.id, "custom-id");
        assert_eq!(checkpoint.name, "Custom checkpoint");
    }

    #[test]
    fn test_checkpoint_serialization() {
        let state = PersistentState::new();
        let checkpoint = Checkpoint::new("Test".to_string(), state);

        let json = serde_json::to_string(&checkpoint).unwrap();
        let parsed: Checkpoint = serde_json::from_str(&json).unwrap();

        assert_eq!(checkpoint.id, parsed.id);
        assert_eq!(checkpoint.name, parsed.name);
    }

    #[test]
    fn test_checkpoint_unique_ids() {
        let state = PersistentState::new();
        let checkpoint1 = Checkpoint::new("First".to_string(), state.clone());
        std::thread::sleep(std::time::Duration::from_millis(5));
        let checkpoint2 = Checkpoint::new("Second".to_string(), state);

        assert_ne!(checkpoint1.id, checkpoint2.id);
    }

    // RecoveryResult tests

    #[test]
    fn test_recovery_result_restored() {
        let result = RecoveryResult::Restored(SessionId(1));

        assert!(result.is_success());
        assert_eq!(result.session_id(), Some(SessionId(1)));
        assert!(result.warnings().is_empty());
        assert!(result.error().is_none());
    }

    #[test]
    fn test_recovery_result_partial_restore() {
        let warnings = vec![
            "Scrollback buffer not restored".to_string(),
            "Context usage reset".to_string(),
        ];
        let result = RecoveryResult::PartialRestore(SessionId(1), warnings);

        assert!(result.is_success());
        assert_eq!(result.session_id(), Some(SessionId(1)));
        assert_eq!(result.warnings().len(), 2);
        assert!(result.error().is_none());
    }

    #[test]
    fn test_recovery_result_failed() {
        let result = RecoveryResult::Failed("Working directory not found".to_string());

        assert!(!result.is_success());
        assert!(result.session_id().is_none());
        assert!(result.warnings().is_empty());
        assert_eq!(result.error(), Some("Working directory not found"));
    }

    #[test]
    fn test_recovery_result_clone() {
        let result = RecoveryResult::Restored(SessionId(1));
        let cloned = result.clone();

        assert!(matches!(cloned, RecoveryResult::Restored(_)));
    }

    #[test]
    fn test_recovery_result_debug() {
        let result = RecoveryResult::Restored(SessionId(42));
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Restored"));
    }
}
