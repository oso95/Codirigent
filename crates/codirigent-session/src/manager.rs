//! Session manager implementation.
//!
//! This module provides the default implementation of the [`SessionManager`]
//! trait, managing session lifecycle, PTY I/O, and event publishing.

use crate::git_status::GitStatusService;
use crate::pty::{spawn_output_reader, PtyHandle};
use crate::session::SessionState;
use anyhow::{anyhow, Context, Result};
use codirigent_core::{
    CodirigentEvent, EventBus, GitRepoInfo, Session, SessionId, SessionManager, SessionStatus,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Default terminal height in rows.
const DEFAULT_PTY_ROWS: u16 = 24;
/// Default terminal width in columns.
const DEFAULT_PTY_COLS: u16 = 80;

/// Default implementation of [`SessionManager`].
///
/// Manages terminal sessions including PTY spawning, I/O handling,
/// and event publishing. Sessions are stored in a HashMap for O(1)
/// lookup by ID.
///
/// # Example
///
/// ```ignore
/// use codirigent_session::DefaultSessionManager;
/// use codirigent_core::{DefaultEventBus, SessionManager};
/// use std::sync::Arc;
///
/// let event_bus = Arc::new(DefaultEventBus::new(16));
/// let mut manager = DefaultSessionManager::new(event_bus);
///
/// let id = manager.create_session(
///     "My Session".to_string(),
///     std::path::PathBuf::from("/tmp"),
/// ).unwrap();
///
/// manager.send_input(id, b"echo hello\n").unwrap();
/// ```
pub struct DefaultSessionManager {
    /// Active sessions indexed by ID.
    ///
    /// Wrapped in Mutex to satisfy Sync requirement. The inner types
    /// (PtyHandle) are Send but not Sync due to raw I/O handles.
    sessions: Mutex<HashMap<SessionId, SessionState>>,
    /// Counter for generating unique session IDs.
    next_id: AtomicU64,
    /// Event bus for publishing session events.
    event_bus: Arc<dyn EventBus>,
    /// Git status detection service.
    git_status: Mutex<GitStatusService>,
}

impl DefaultSessionManager {
    /// Create a new session manager.
    ///
    /// # Arguments
    ///
    /// * `event_bus` - The event bus for publishing session events
    pub fn new(event_bus: Arc<dyn EventBus>) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            event_bus,
            git_status: Mutex::new(GitStatusService::new()),
        }
    }

    /// Acquire the sessions lock, recovering from mutex poisoning if needed.
    ///
    /// If another thread panicked while holding the lock, this method will
    /// recover the lock and log a warning. The data may be in an inconsistent
    /// state, but we prioritize availability over strict consistency.
    fn lock_sessions(&self) -> MutexGuard<'_, HashMap<SessionId, SessionState>> {
        self.sessions.lock().unwrap_or_else(|poisoned| {
            warn!("Sessions mutex was poisoned, recovering lock");
            poisoned.into_inner()
        })
    }

    /// Generate a unique session ID.
    fn next_session_id(&self) -> SessionId {
        SessionId(self.next_id.fetch_add(1, Ordering::SeqCst))
    }

    /// Execute a function with mutable access to a session state.
    ///
    /// This method provides safe access to session state by acquiring
    /// the internal lock and calling the provided closure.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID to access
    /// * `f` - Function to execute with the session state
    ///
    /// # Returns
    ///
    /// `None` if the session doesn't exist, otherwise the result of `f`.
    pub fn with_session_state_mut<T, F>(&self, id: SessionId, f: F) -> Option<T>
    where
        F: FnOnce(&mut SessionState) -> T,
    {
        let mut sessions = self.lock_sessions();
        sessions.get_mut(&id).map(f)
    }

    /// Execute a function with immutable access to a session state.
    ///
    /// This method provides safe access to session state by acquiring
    /// the internal lock and calling the provided closure.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID to access
    /// * `f` - Function to execute with the session state
    ///
    /// # Returns
    ///
    /// `None` if the session doesn't exist, otherwise the result of `f`.
    pub fn with_session_state<T, F>(&self, id: SessionId, f: F) -> Option<T>
    where
        F: FnOnce(&SessionState) -> T,
    {
        let sessions = self.lock_sessions();
        sessions.get(&id).map(f)
    }

    /// Drain output from a session's channel (non-blocking).
    ///
    /// Collects all available output from the PTY output channel
    /// without blocking. Returns `None` if no output is available
    /// or the session doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID to drain output from
    pub fn try_drain_output(&self, id: SessionId) -> Option<Vec<u8>> {
        let mut sessions = self.lock_sessions();
        let state = sessions.get_mut(&id)?;
        let mut output = Vec::new();

        while let Ok(data) = state.output_rx.try_recv() {
            output.extend(data);
        }

        if output.is_empty() {
            None
        } else {
            Some(output)
        }
    }

    /// Publish an event to the event bus.
    fn publish(&self, event: CodirigentEvent) {
        self.event_bus.publish(event);
    }

    /// Get the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.lock_sessions().len()
    }

    /// Check if a session exists.
    pub fn session_exists(&self, id: SessionId) -> bool {
        self.lock_sessions().contains_key(&id)
    }

    /// Get all session IDs.
    pub fn session_ids(&self) -> Vec<SessionId> {
        self.lock_sessions().keys().copied().collect()
    }

    /// Get the child PID for a session.
    ///
    /// Returns the process ID of the PTY child process for the given session.
    pub fn get_child_pid(&self, id: SessionId) -> Option<u32> {
        self.lock_sessions().get(&id).map(|s| s.child_pid())
    }

    /// Update the working directory for a session (detected via OSC 7).
    ///
    /// If the new directory differs from the current one, updates the session,
    /// invalidates the git cache for the old repo root, and publishes a
    /// `WorkingDirectoryChanged` event.
    ///
    /// Returns `true` if the directory actually changed.
    pub fn update_working_directory(&self, id: SessionId, new_dir: PathBuf) -> bool {
        // Canonicalize the new path so forward/backslash differences on Windows
        // don't cause spurious "changed" detections.
        let new_dir = std::fs::canonicalize(&new_dir).unwrap_or(new_dir);

        let old_dir = {
            let mut sessions = self.lock_sessions();
            let state = match sessions.get_mut(&id) {
                Some(s) => s,
                None => return false,
            };

            // Also canonicalize the stored path for comparison
            let current = std::fs::canonicalize(&state.session.working_directory)
                .unwrap_or_else(|_| state.session.working_directory.clone());
            if current == new_dir {
                return false;
            }

            let old = state.session.working_directory.clone();
            state.session.working_directory = new_dir.clone();

            // Clear stale git info so the next refresh picks up the new repo
            if let Some(ref info) = state.session.git_info {
                let repo_root = info.repo_root.clone();
                drop(sessions); // release session lock before git lock
                self.git_status
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .invalidate(&repo_root);
            } else {
                drop(sessions);
            }

            old
        };

        info!(%id, old=?old_dir, new=?new_dir, "Session working directory changed (OSC 7)");

        self.publish(CodirigentEvent::WorkingDirectoryChanged {
            id,
            old_dir,
            new_dir,
        });

        true
    }

    /// Refresh git status for a session.
    ///
    /// Detects or refreshes git repository information for the session's
    /// working directory. Uses cached results within the 3-second TTL.
    ///
    /// Returns the updated git info, or None if the session doesn't exist
    /// or isn't in a git repository.
    pub fn refresh_git_status(&self, id: SessionId) -> Option<GitRepoInfo> {
        let working_dir = {
            let sessions = self.lock_sessions();
            sessions.get(&id)?.session.working_directory.clone()
        };

        let info = self
            .git_status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .detect_cached(&working_dir, Duration::from_secs(3));

        // Update session
        let mut sessions = self.lock_sessions();
        if let Some(state) = sessions.get_mut(&id) {
            state.session.git_info = info.clone();
        }

        info
    }
}

impl SessionManager for DefaultSessionManager {
    fn list_sessions(&self) -> Vec<Session> {
        self.lock_sessions()
            .values()
            .map(|s| s.session.clone())
            .collect()
    }

    fn get_session(&self, id: SessionId) -> Option<Session> {
        self.lock_sessions().get(&id).map(|s| s.session.clone())
    }

    fn create_session(&self, name: String, working_dir: PathBuf) -> Result<SessionId> {
        let id = self.next_session_id();
        info!(%id, %name, ?working_dir, "Creating session");

        // Validate working directory exists and is a directory
        if !working_dir.exists() {
            return Err(anyhow!(
                "Working directory does not exist: {}",
                working_dir.display()
            ));
        }
        if !working_dir.is_dir() {
            return Err(anyhow!(
                "Working directory path is not a directory: {}",
                working_dir.display()
            ));
        }

        // Spawn PTY with default terminal size
        let mut pty = PtyHandle::spawn(&working_dir, DEFAULT_PTY_ROWS, DEFAULT_PTY_COLS, &[])
            .context("Failed to spawn PTY")?;

        // Take reader and spawn output task
        let reader = pty
            .take_reader()
            .ok_or_else(|| anyhow!("Failed to get PTY reader"))?;
        let output_rx = spawn_output_reader(reader);

        // Create session metadata
        let mut session = Session::new(id, name, working_dir.clone());

        // Detect git info for the working directory
        session.git_info = self
            .git_status
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .detect(&working_dir);

        // Create session state
        let state = SessionState::new(session, pty, output_rx);
        self.lock_sessions().insert(id, state);

        // Publish event
        self.publish(CodirigentEvent::SessionCreated { id });

        Ok(id)
    }

    fn close_session(&self, id: SessionId) -> Result<()> {
        info!(%id, "Closing session");

        let _state = self
            .lock_sessions()
            .remove(&id)
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;

        // PTY and output_rx will be dropped, cleaning up resources

        self.publish(CodirigentEvent::SessionClosed { id });

        Ok(())
    }

    fn send_input(&self, id: SessionId, input: &[u8]) -> Result<()> {
        let mut sessions = self.lock_sessions();
        let state = sessions
            .get_mut(&id)
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;

        state
            .pty
            .send_input(input)
            .context("Failed to send input to PTY")?;

        Ok(())
    }

    fn resize(&self, id: SessionId, rows: u16, cols: u16) -> Result<()> {
        let mut sessions = self.lock_sessions();
        let state = sessions
            .get_mut(&id)
            .ok_or_else(|| anyhow!("Session not found: {}", id))?;

        state
            .pty
            .resize(rows, cols)
            .context("Failed to resize PTY")?;

        Ok(())
    }

    fn update_status(&self, id: SessionId, status: SessionStatus) {
        let mut sessions = self.lock_sessions();
        if let Some(state) = sessions.get_mut(&id) {
            let old = state.status();
            if old != status {
                debug!(%id, ?old, ?status, "Session status changed");
                state.set_status(status);
                // Drop lock before publishing to avoid deadlock
                drop(sessions);
                self.publish(CodirigentEvent::SessionStatusChanged {
                    id,
                    old,
                    new: status,
                });
            }
        }
    }

    fn rename_session(&self, id: SessionId, new_name: String) -> Result<()> {
        info!(%id, %new_name, "Renaming session");

        let old_name = {
            let mut sessions = self.lock_sessions();
            let state = sessions
                .get_mut(&id)
                .ok_or_else(|| anyhow!("Session not found: {}", id))?;

            let old_name = state.session.name.clone();
            state.session.name = new_name.clone();
            old_name
        };

        debug!(%id, %old_name, %new_name, "Session renamed successfully");

        self.publish(CodirigentEvent::SessionRenamed {
            id,
            old_name,
            new_name,
        });

        Ok(())
    }

    fn set_session_group(
        &self,
        id: SessionId,
        group: Option<String>,
        color: Option<String>,
    ) -> Result<()> {
        info!(%id, ?group, ?color, "Setting session group");

        {
            let mut sessions = self.lock_sessions();
            let state = sessions
                .get_mut(&id)
                .ok_or_else(|| anyhow!("Session not found: {}", id))?;

            state.session.group = group.clone();
            state.session.color = color.clone();
        }

        debug!(%id, ?group, ?color, "Session group updated successfully");

        self.publish(CodirigentEvent::SessionGroupChanged { id, group, color });

        Ok(())
    }

    fn update_context_usage(&self, id: SessionId, usage: Option<f32>) {
        let mut sessions = self.lock_sessions();
        if let Some(state) = sessions.get_mut(&id) {
            state.session.context_usage = usage;
        }
    }

    fn get_context_file_path(&self, _id: SessionId) -> Option<PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codirigent_core::DefaultEventBus;

    fn create_manager() -> DefaultSessionManager {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        DefaultSessionManager::new(event_bus)
    }

    fn create_manager_with_bus() -> (DefaultSessionManager, Arc<DefaultEventBus>) {
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let manager = DefaultSessionManager::new(event_bus.clone());
        (manager, event_bus)
    }

    #[test]
    fn test_new_manager() {
        let manager = create_manager();
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_create_session() {
        let manager = create_manager();

        let id = manager
            .create_session("Test Session".to_string(), std::env::temp_dir())
            .unwrap();

        assert!(manager.get_session(id).is_some());
        assert_eq!(manager.list_sessions().len(), 1);
        assert_eq!(manager.session_count(), 1);
    }

    #[test]
    fn test_create_multiple_sessions() {
        let manager = create_manager();

        let id1 = manager
            .create_session("Session 1".to_string(), std::env::temp_dir())
            .unwrap();
        let id2 = manager
            .create_session("Session 2".to_string(), std::env::temp_dir())
            .unwrap();
        let id3 = manager
            .create_session("Session 3".to_string(), std::env::temp_dir())
            .unwrap();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_eq!(manager.session_count(), 3);
    }

    #[test]
    fn test_create_session_publishes_event() {
        let (manager, event_bus) = create_manager_with_bus();
        let mut rx = event_bus.subscribe();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        let event = rx.try_recv().unwrap();
        assert!(
            matches!(event, CodirigentEvent::SessionCreated { id: created_id } if created_id == id)
        );
    }

    #[test]
    fn test_close_session() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        manager.close_session(id).unwrap();
        assert!(manager.get_session(id).is_none());
        assert_eq!(manager.session_count(), 0);
    }

    #[test]
    fn test_close_session_publishes_event() {
        let (manager, event_bus) = create_manager_with_bus();
        let mut rx = event_bus.subscribe();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        // Consume create event
        let _ = rx.try_recv();

        manager.close_session(id).unwrap();

        let event = rx.try_recv().unwrap();
        assert!(
            matches!(event, CodirigentEvent::SessionClosed { id: closed_id } if closed_id == id)
        );
    }

    #[test]
    fn test_close_nonexistent_session() {
        let manager = create_manager();

        let result = manager.close_session(SessionId(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_session() {
        let manager = create_manager();

        let id = manager
            .create_session("Test Session".to_string(), std::env::temp_dir())
            .unwrap();

        let session = manager.get_session(id).unwrap();
        assert_eq!(session.id, id);
        assert_eq!(session.name, "Test Session");
    }

    #[test]
    fn test_get_nonexistent_session() {
        let manager = create_manager();
        assert!(manager.get_session(SessionId(999)).is_none());
    }

    #[test]
    fn test_list_sessions() {
        let manager = create_manager();

        manager
            .create_session("Session 1".to_string(), std::env::temp_dir())
            .unwrap();
        manager
            .create_session("Session 2".to_string(), std::env::temp_dir())
            .unwrap();

        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_send_input() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        let result = manager.send_input(id, b"echo hello\n");
        assert!(result.is_ok());
    }

    #[test]
    fn test_send_input_nonexistent_session() {
        let manager = create_manager();

        let result = manager.send_input(SessionId(999), b"test");
        assert!(result.is_err());
    }

    #[test]
    fn test_resize() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        let result = manager.resize(id, 48, 120);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resize_nonexistent_session() {
        let manager = create_manager();

        let result = manager.resize(SessionId(999), 48, 120);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_status() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        manager.update_status(id, SessionStatus::Working);

        let session = manager.get_session(id).unwrap();
        assert_eq!(session.status, SessionStatus::Working);
    }

    #[test]
    fn test_update_status_publishes_event() {
        let (manager, event_bus) = create_manager_with_bus();
        let mut rx = event_bus.subscribe();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        // Consume create event
        let _ = rx.try_recv();

        manager.update_status(id, SessionStatus::Working);

        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            CodirigentEvent::SessionStatusChanged {
                id: changed_id,
                old: SessionStatus::Idle,
                new: SessionStatus::Working
            } if changed_id == id
        ));
    }

    #[test]
    fn test_update_status_no_event_if_unchanged() {
        let (manager, event_bus) = create_manager_with_bus();
        let mut rx = event_bus.subscribe();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        // Consume create event
        let _ = rx.try_recv();

        // Set to Idle (same as default) - should not publish event
        manager.update_status(id, SessionStatus::Idle);

        // Channel should be empty
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_update_status_nonexistent_session() {
        let manager = create_manager();

        // Should not panic
        manager.update_status(SessionId(999), SessionStatus::Working);
    }

    #[test]
    fn test_rename_session() {
        let manager = create_manager();

        let id = manager
            .create_session("Original".to_string(), std::env::temp_dir())
            .unwrap();

        manager.rename_session(id, "Renamed".to_string()).unwrap();

        let session = manager.get_session(id).unwrap();
        assert_eq!(session.name, "Renamed");
    }

    #[test]
    fn test_rename_session_publishes_event() {
        let (manager, event_bus) = create_manager_with_bus();
        let mut rx = event_bus.subscribe();

        let id = manager
            .create_session("Original".to_string(), std::env::temp_dir())
            .unwrap();

        // Consume create event
        let _ = rx.try_recv();

        manager.rename_session(id, "Renamed".to_string()).unwrap();

        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            CodirigentEvent::SessionRenamed {
                id: renamed_id,
                old_name,
                new_name
            } if renamed_id == id && old_name == "Original" && new_name == "Renamed"
        ));
    }

    #[test]
    fn test_rename_nonexistent_session() {
        let manager = create_manager();

        let result = manager.rename_session(SessionId(999), "New".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_set_session_group() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        manager
            .set_session_group(
                id,
                Some("my-project".to_string()),
                Some("#ff0000".to_string()),
            )
            .unwrap();

        let session = manager.get_session(id).unwrap();
        assert_eq!(session.group, Some("my-project".to_string()));
        assert_eq!(session.color, Some("#ff0000".to_string()));
    }

    #[test]
    fn test_set_session_group_clear() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        // Set group
        manager
            .set_session_group(id, Some("group".to_string()), Some("#000".to_string()))
            .unwrap();

        // Clear group
        manager.set_session_group(id, None, None).unwrap();

        let session = manager.get_session(id).unwrap();
        assert!(session.group.is_none());
        assert!(session.color.is_none());
    }

    #[test]
    fn test_set_session_group_publishes_event() {
        let (manager, event_bus) = create_manager_with_bus();
        let mut rx = event_bus.subscribe();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        // Consume create event
        let _ = rx.try_recv();

        manager
            .set_session_group(id, Some("backend".to_string()), Some("#00ff00".to_string()))
            .unwrap();

        let event = rx.try_recv().unwrap();
        assert!(matches!(
            event,
            CodirigentEvent::SessionGroupChanged {
                id: changed_id,
                group: Some(g),
                color: Some(c)
            } if changed_id == id && g == "backend" && c == "#00ff00"
        ));
    }

    #[test]
    fn test_set_session_group_nonexistent() {
        let manager = create_manager();

        let result = manager.set_session_group(SessionId(999), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_with_session_state() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        let session_id = manager.with_session_state(id, |state| state.id());
        assert_eq!(session_id, Some(id));
    }

    #[test]
    fn test_with_session_state_mut() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        manager.with_session_state_mut(id, |state| {
            state.set_status(SessionStatus::Working);
        });

        let session = manager.get_session(id).unwrap();
        assert_eq!(session.status, SessionStatus::Working);
    }

    #[test]
    fn test_session_exists() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        assert!(manager.session_exists(id));
        assert!(!manager.session_exists(SessionId(999)));
    }

    #[test]
    fn test_session_ids() {
        let manager = create_manager();

        let id1 = manager
            .create_session("Session 1".to_string(), std::env::temp_dir())
            .unwrap();
        let id2 = manager
            .create_session("Session 2".to_string(), std::env::temp_dir())
            .unwrap();

        let ids = manager.session_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn test_get_child_pid() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        let pid = manager.get_child_pid(id).unwrap();
        assert!(pid > 0);
    }

    #[test]
    fn test_get_child_pid_nonexistent() {
        let manager = create_manager();
        assert!(manager.get_child_pid(SessionId(999)).is_none());
    }

    #[tokio::test]
    async fn test_try_drain_output() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        // Send a command
        manager.send_input(id, b"echo test_drain\n").unwrap();

        // Wait for output
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Drain output
        let output = manager.try_drain_output(id);

        // Should have some output (even if it's just the prompt)
        // Note: output may or may not contain our echo depending on timing
        // The important thing is that the drain works without blocking
        let _ = output;
    }

    #[test]
    fn test_try_drain_output_nonexistent() {
        let manager = create_manager();
        assert!(manager.try_drain_output(SessionId(999)).is_none());
    }

    #[test]
    fn test_try_drain_output_empty() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        // Drain immediately - might be empty or have shell prompt
        // Just verify it doesn't panic or block
        let _ = manager.try_drain_output(id);
    }

    #[test]
    fn test_session_increments_ids() {
        let manager = create_manager();

        let id1 = manager
            .create_session("1".to_string(), std::env::temp_dir())
            .unwrap();
        let id2 = manager
            .create_session("2".to_string(), std::env::temp_dir())
            .unwrap();
        let id3 = manager
            .create_session("3".to_string(), std::env::temp_dir())
            .unwrap();

        assert_eq!(id1.0 + 1, id2.0);
        assert_eq!(id2.0 + 1, id3.0);
    }

    #[test]
    fn test_create_session_with_nonexistent_working_dir() {
        let manager = create_manager();

        let result = manager.create_session(
            "Test".to_string(),
            PathBuf::from("/nonexistent/path/that/does/not/exist"),
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("does not exist"));
    }

    #[test]
    fn test_create_session_with_file_as_working_dir() {
        use std::io::Write;

        let manager = create_manager();

        // Create a temporary file (not a directory)
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("dirigent_test_file_{}", std::process::id()));
        {
            let mut file = std::fs::File::create(&temp_file).unwrap();
            file.write_all(b"test").unwrap();
        }

        let result = manager.create_session("Test".to_string(), temp_file.clone());

        // Clean up
        let _ = std::fs::remove_file(&temp_file);

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not a directory"));
    }

    #[test]
    fn test_update_context_usage() {
        let manager = create_manager();

        let id = manager
            .create_session("Test".to_string(), std::env::temp_dir())
            .unwrap();

        assert!(manager.get_session(id).unwrap().context_usage.is_none());

        manager.update_context_usage(id, Some(0.65));
        let session = manager.get_session(id).unwrap();
        assert!((session.context_usage.unwrap() - 0.65).abs() < f32::EPSILON);

        manager.update_context_usage(id, Some(0.85));
        let session = manager.get_session(id).unwrap();
        assert!((session.context_usage.unwrap() - 0.85).abs() < f32::EPSILON);

        manager.update_context_usage(id, None);
        assert!(manager.get_session(id).unwrap().context_usage.is_none());
    }

    #[test]
    fn test_update_context_usage_nonexistent() {
        let manager = create_manager();
        manager.update_context_usage(SessionId(999), Some(0.5));
    }
}
