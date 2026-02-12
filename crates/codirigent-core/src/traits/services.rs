//! Core service traits for the Codirigent application.
//!
//! This module defines the trait contracts that govern inter-crate
//! communication. Each trait represents a service that can be
//! implemented by different crates.
//!
//! ## Traits
//!
//! - [`EventBus`]: Cross-module event publication and subscription
//! - [`SessionManager`]: Session lifecycle management
//! - [`ProcessMonitor`]: Process state monitoring
//! - [`StorageService`]: File-based persistence

use crate::events::CodirigentEvent;
use crate::types::*;
use anyhow::Result;
use std::path::Path;
use tokio::sync::broadcast;

/// Event bus for cross-module communication.
///
/// Modules publish events here, and other modules subscribe to receive them.
/// This enables loose coupling between components.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
///
/// # Example
///
/// ```ignore
/// use codirigent_core::{EventBus, CodirigentEvent, SessionId};
///
/// async fn example(bus: &impl EventBus) {
///     let mut rx = bus.subscribe();
///
///     // Publish an event
///     bus.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });
///
///     // Receive events
///     while let Ok(event) = rx.recv().await {
///         println!("Received: {:?}", event);
///     }
/// }
/// ```
pub trait EventBus: Send + Sync {
    /// Subscribe to all events.
    ///
    /// Returns a receiver that will get all published events.
    /// Multiple subscribers can exist simultaneously.
    fn subscribe(&self) -> broadcast::Receiver<CodirigentEvent>;

    /// Publish an event to all subscribers.
    ///
    /// Events are broadcast to all current subscribers. If there are
    /// no subscribers, the event is silently dropped.
    fn publish(&self, event: CodirigentEvent);
}

/// Session management service.
///
/// Implemented by `dirigent-session`, consumed by `dirigent-ui` and others.
/// This trait defines the contract for managing terminal sessions.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
///
/// # Implementors
///
/// The primary implementation is in the `dirigent-session` crate.
pub trait SessionManager: Send + Sync {
    /// Get all active sessions.
    ///
    /// Returns a list of cloned sessions currently managed.
    fn list_sessions(&self) -> Vec<Session>;

    /// Get a specific session by ID.
    ///
    /// Returns `None` if no session with the given ID exists.
    fn get_session(&self, id: SessionId) -> Option<Session>;

    /// Create a new session with the given name and working directory.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the session
    /// * `working_dir` - Initial working directory for the shell
    /// * `shell` - Optional shell name (e.g. "zsh", "bash"). `None` = auto-detect.
    ///
    /// # Returns
    ///
    /// The ID of the newly created session, or an error if creation failed.
    fn create_session(
        &self,
        name: String,
        working_dir: std::path::PathBuf,
        shell: Option<String>,
    ) -> Result<SessionId>;

    /// Close and cleanup a session.
    ///
    /// This terminates the PTY process and removes the session from management.
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist or cleanup fails.
    fn close_session(&self, id: SessionId) -> Result<()>;

    /// Send input to a session's PTY.
    ///
    /// The input bytes are written directly to the PTY's stdin.
    ///
    /// # Arguments
    ///
    /// * `id` - The session to send input to
    /// * `input` - Raw bytes to write
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist or writing fails.
    fn send_input(&self, id: SessionId, input: &[u8]) -> Result<()>;

    /// Resize a session's PTY.
    ///
    /// # Arguments
    ///
    /// * `id` - The session to resize
    /// * `rows` - New row count
    /// * `cols` - New column count
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist or resize fails.
    fn resize(&self, id: SessionId, rows: u16, cols: u16) -> Result<()>;

    /// Update session status (called by detector).
    ///
    /// This is called by the process monitor when it detects a status change.
    fn update_status(&self, id: SessionId, status: SessionStatus);

    /// Rename a session.
    ///
    /// # Arguments
    ///
    /// * `id` - The session to rename
    /// * `new_name` - The new name
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist.
    fn rename_session(&self, id: SessionId, new_name: String) -> Result<()>;

    /// Set session group and color.
    ///
    /// Groups are used for visual organization in the UI.
    ///
    /// # Arguments
    ///
    /// * `id` - The session to modify
    /// * `group` - Group name (None to ungroup)
    /// * `color` - Group color (None for default)
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist.
    fn set_session_group(
        &self,
        id: SessionId,
        group: Option<String>,
        color: Option<String>,
    ) -> Result<()>;

    /// Update context usage for a session.
    ///
    /// Sets the context window usage percentage for a session.
    /// This is called when context tracking detects a usage change.
    fn update_context_usage(&self, id: SessionId, usage: Option<f32>);

    /// Get the context file path for a session.
    ///
    /// Returns the path to the session's context file used for
    /// file-based IPC with CLI hooks, or `None` if the session
    /// doesn't exist or has no context file.
    fn get_context_file_path(&self, id: SessionId) -> Option<std::path::PathBuf>;
}

/// Process monitoring service.
///
/// Implemented by `dirigent-detector`, used to track session status
/// based on process state and output patterns.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
///
/// # Implementors
///
/// The primary implementation is in the `dirigent-detector` crate.
pub trait ProcessMonitor: Send + Sync {
    /// Start monitoring a session's PTY process.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to monitor
    /// * `pty_pid` - The process ID of the PTY
    ///
    /// # Errors
    ///
    /// Returns an error if monitoring cannot be started.
    fn start_monitoring(&mut self, session_id: SessionId, pty_pid: u32) -> Result<()>;

    /// Stop monitoring a session.
    ///
    /// This should be called when a session is closed.
    fn stop_monitoring(&mut self, session_id: SessionId);

    /// Get current detected status for a session.
    ///
    /// Returns `None` if the session is not being monitored.
    fn get_status(&self, session_id: SessionId) -> Option<SessionStatus>;

    /// Add a custom pattern to detect.
    ///
    /// Patterns are used to detect when the CLI is waiting for input.
    fn add_pattern(&mut self, pattern: String);

    /// Remove a custom pattern.
    fn remove_pattern(&mut self, pattern: &str);
}

/// Storage service for file-based persistence.
///
/// Handles reading/writing to the `.codirigent` directory.
/// All state is stored as JSON files for portability and debuggability.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
///
/// # File Structure
///
/// ```text
/// .codirigent/
/// ├── config.json    # Project configuration
/// ├── state.json     # Runtime state
/// ├── queue.json     # Task queue
/// └── tasks/         # Individual task files
///     ├── task-001.json
///     └── task-002.json
/// ```
pub trait StorageService: Send + Sync {
    /// Get the `.codirigent` directory path.
    ///
    /// Returns the path to the project's `.codirigent` directory.
    fn codirigent_dir(&self) -> &Path;

    /// Load application state from disk.
    ///
    /// # Returns
    ///
    /// The loaded state, or a default state if none exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the state file exists but cannot be read.
    fn load_state(&self) -> Result<AppState>;

    /// Save application state to disk.
    ///
    /// The state is written atomically to prevent corruption.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    fn save_state(&self, state: &AppState) -> Result<()>;

    /// Load a specific task by ID.
    ///
    /// # Returns
    ///
    /// The task if found, `None` if the task doesn't exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the task file exists but cannot be read.
    fn load_task(&self, id: &TaskId) -> Result<Option<Task>>;

    /// Save a task to disk.
    ///
    /// Creates or updates the task file.
    ///
    /// # Errors
    ///
    /// Returns an error if writing fails.
    fn save_task(&self, task: &Task) -> Result<()>;

    /// List all task IDs.
    ///
    /// Returns a list of all task IDs found in the tasks directory.
    ///
    /// # Errors
    ///
    /// Returns an error if reading the directory fails.
    fn list_task_ids(&self) -> Result<Vec<TaskId>>;

    /// Delete a task.
    ///
    /// Removes the task file from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be deleted.
    fn delete_task(&self, id: &TaskId) -> Result<()>;
}

/// Broadcast service for sending messages to multiple sessions.
///
/// This trait defines the contract for broadcasting messages to AI coding
/// sessions. Messages can include template variables that are expanded
/// before sending.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync` to allow sharing across threads.
///
/// # Example
///
/// ```ignore
/// use codirigent_core::{BroadcastService, BroadcastVariables, SessionId};
///
/// fn broadcast_update(service: &mut dyn BroadcastService) {
///     let vars = BroadcastVariables::new()
///         .with_project("my-app".to_string());
///
///     // Send to specific sessions
///     let targets = vec![SessionId(1), SessionId(2)];
///     service.send_with_variables(
///         "API changed in $PROJECT, please update",
///         targets,
///         vars,
///     ).unwrap();
///
///     // Or send to all sessions
///     service.send_to_all("General announcement").unwrap();
/// }
/// ```
pub trait BroadcastService: Send + Sync {
    /// Send a message to specified sessions.
    ///
    /// # Arguments
    ///
    /// * `content` - The message content to send
    /// * `targets` - List of session IDs to send to
    ///
    /// # Returns
    ///
    /// The broadcast ID for tracking delivery status.
    fn send(
        &mut self,
        content: &str,
        targets: Vec<SessionId>,
    ) -> Result<crate::broadcast::BroadcastId>;

    /// Send with template variable expansion.
    ///
    /// Variables in the template are expanded before sending:
    /// - `$SESSION_NAME` - Session name
    /// - `$WORKTREE` - Worktree path
    /// - `$PROJECT` - Project name
    /// - Custom variables via the `custom` HashMap
    ///
    /// # Arguments
    ///
    /// * `template` - Template string with variable placeholders
    /// * `targets` - List of session IDs to send to
    /// * `variables` - Variables to expand in the template
    ///
    /// # Returns
    ///
    /// The broadcast ID for tracking delivery status.
    fn send_with_variables(
        &mut self,
        template: &str,
        targets: Vec<SessionId>,
        variables: crate::broadcast::BroadcastVariables,
    ) -> Result<crate::broadcast::BroadcastId>;

    /// Send with priority level.
    ///
    /// # Arguments
    ///
    /// * `content` - The message content to send
    /// * `targets` - List of session IDs to send to
    /// * `priority` - Message priority level
    ///
    /// # Returns
    ///
    /// The broadcast ID for tracking delivery status.
    fn send_with_priority(
        &mut self,
        content: &str,
        targets: Vec<SessionId>,
        priority: crate::broadcast::BroadcastPriority,
    ) -> Result<crate::broadcast::BroadcastId>;

    /// Send to all active sessions.
    ///
    /// This requires access to the session list to determine active sessions.
    ///
    /// # Arguments
    ///
    /// * `content` - The message content to send
    ///
    /// # Returns
    ///
    /// The broadcast ID for tracking delivery status.
    fn send_to_all(&mut self, content: &str) -> Result<crate::broadcast::BroadcastId>;

    /// Get broadcast history (most recent first).
    ///
    /// Returns a slice of all broadcast history entries, ordered by
    /// creation time with most recent first.
    fn history(&self) -> &[crate::broadcast::BroadcastHistoryEntry];

    /// Get a specific broadcast by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The broadcast ID to look up
    ///
    /// # Returns
    ///
    /// The broadcast message if found.
    fn get_broadcast(
        &self,
        id: crate::broadcast::BroadcastId,
    ) -> Option<&crate::broadcast::BroadcastMessage>;

    /// Clear broadcast history.
    ///
    /// Removes all entries from the history. Active broadcasts may still
    /// be tracked internally.
    fn clear_history(&mut self);

    /// Retry failed deliveries for a broadcast.
    ///
    /// Attempts to re-deliver the message to sessions that previously
    /// failed to receive it.
    ///
    /// # Arguments
    ///
    /// * `id` - The broadcast ID to retry
    ///
    /// # Errors
    ///
    /// Returns an error if the broadcast is not found.
    fn retry_failed(&mut self, id: crate::broadcast::BroadcastId) -> Result<()>;

    /// Get the next broadcast ID.
    ///
    /// Used internally to generate unique broadcast IDs.
    fn next_id(&mut self) -> crate::broadcast::BroadcastId;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test that traits have the expected methods (compile-time check)
    // These tests verify trait object safety and method signatures

    #[test]
    fn test_event_bus_trait_is_object_safe() {
        // This compiles only if EventBus is object-safe
        fn _takes_event_bus(_: &dyn EventBus) {}
    }

    #[test]
    fn test_session_manager_trait_is_object_safe() {
        // This compiles only if SessionManager is object-safe
        fn _takes_session_manager(_: &dyn SessionManager) {}
    }

    #[test]
    fn test_process_monitor_trait_is_object_safe() {
        // This compiles only if ProcessMonitor is object-safe
        fn _takes_process_monitor(_: &dyn ProcessMonitor) {}
    }

    #[test]
    fn test_storage_service_trait_is_object_safe() {
        // This compiles only if StorageService is object-safe
        fn _takes_storage_service(_: &dyn StorageService) {}
    }

    #[test]
    fn test_broadcast_service_trait_is_object_safe() {
        // This compiles only if BroadcastService is object-safe
        fn _takes_broadcast_service(_: &dyn BroadcastService) {}
    }

    // Mock implementations for testing trait contracts
    struct MockEventBus;

    impl EventBus for MockEventBus {
        fn subscribe(&self) -> broadcast::Receiver<CodirigentEvent> {
            let (tx, rx) = broadcast::channel(1);
            drop(tx);
            rx
        }

        fn publish(&self, _event: CodirigentEvent) {}
    }

    #[test]
    fn test_mock_event_bus_compiles() {
        let bus = MockEventBus;
        let _rx = bus.subscribe();
        bus.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });
    }

    struct MockSessionManager {
        sessions: std::sync::Mutex<Vec<Session>>,
    }

    impl SessionManager for MockSessionManager {
        fn list_sessions(&self) -> Vec<Session> {
            self.sessions.lock().unwrap().clone()
        }

        fn get_session(&self, id: SessionId) -> Option<Session> {
            self.sessions
                .lock()
                .unwrap()
                .iter()
                .find(|s| s.id == id)
                .cloned()
        }

        fn create_session(
            &self,
            name: String,
            working_dir: std::path::PathBuf,
            _shell: Option<String>,
        ) -> Result<SessionId> {
            let mut sessions = self.sessions.lock().unwrap();
            let id = SessionId(sessions.len() as u64);
            sessions.push(Session::new(id, name, working_dir));
            Ok(id)
        }

        fn close_session(&self, id: SessionId) -> Result<()> {
            self.sessions.lock().unwrap().retain(|s| s.id != id);
            Ok(())
        }

        fn send_input(&self, _id: SessionId, _input: &[u8]) -> Result<()> {
            Ok(())
        }

        fn resize(&self, _id: SessionId, _rows: u16, _cols: u16) -> Result<()> {
            Ok(())
        }

        fn update_status(&self, id: SessionId, status: SessionStatus) {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.iter_mut().find(|s| s.id == id) {
                session.status = status;
            }
        }

        fn rename_session(&self, id: SessionId, new_name: String) -> Result<()> {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.iter_mut().find(|s| s.id == id) {
                session.name = new_name;
            }
            Ok(())
        }

        fn set_session_group(
            &self,
            id: SessionId,
            group: Option<String>,
            color: Option<String>,
        ) -> Result<()> {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.iter_mut().find(|s| s.id == id) {
                session.group = group;
                session.color = color;
            }
            Ok(())
        }

        fn update_context_usage(&self, id: SessionId, usage: Option<f32>) {
            let mut sessions = self.sessions.lock().unwrap();
            if let Some(session) = sessions.iter_mut().find(|s| s.id == id) {
                session.context_usage = usage;
            }
        }

        fn get_context_file_path(&self, _id: SessionId) -> Option<std::path::PathBuf> {
            None
        }
    }

    #[test]
    fn test_mock_session_manager_create_and_list() {
        let manager = MockSessionManager {
            sessions: std::sync::Mutex::new(Vec::new()),
        };

        let id = manager
            .create_session("Test".to_string(), std::path::PathBuf::from("/tmp"), None)
            .unwrap();
        assert_eq!(id, SessionId(0));

        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "Test");
    }

    #[test]
    fn test_mock_session_manager_get_session() {
        let manager = MockSessionManager {
            sessions: std::sync::Mutex::new(Vec::new()),
        };

        let id = manager
            .create_session("Test".to_string(), std::path::PathBuf::from("/tmp"), None)
            .unwrap();

        assert!(manager.get_session(id).is_some());
        assert!(manager.get_session(SessionId(999)).is_none());
    }

    #[test]
    fn test_mock_session_manager_close_session() {
        let manager = MockSessionManager {
            sessions: std::sync::Mutex::new(Vec::new()),
        };

        let id = manager
            .create_session("Test".to_string(), std::path::PathBuf::from("/tmp"), None)
            .unwrap();
        assert_eq!(manager.list_sessions().len(), 1);

        manager.close_session(id).unwrap();
        assert_eq!(manager.list_sessions().len(), 0);
    }

    #[test]
    fn test_mock_session_manager_update_status() {
        let manager = MockSessionManager {
            sessions: std::sync::Mutex::new(Vec::new()),
        };

        let id = manager
            .create_session("Test".to_string(), std::path::PathBuf::from("/tmp"), None)
            .unwrap();

        manager.update_status(id, SessionStatus::Working);
        assert_eq!(
            manager.get_session(id).unwrap().status,
            SessionStatus::Working
        );
    }

    #[test]
    fn test_mock_session_manager_rename() {
        let manager = MockSessionManager {
            sessions: std::sync::Mutex::new(Vec::new()),
        };

        let id = manager
            .create_session("Old".to_string(), std::path::PathBuf::from("/tmp"), None)
            .unwrap();

        manager.rename_session(id, "New".to_string()).unwrap();
        assert_eq!(manager.get_session(id).unwrap().name, "New");
    }

    #[test]
    fn test_mock_session_manager_set_group() {
        let manager = MockSessionManager {
            sessions: std::sync::Mutex::new(Vec::new()),
        };

        let id = manager
            .create_session("Test".to_string(), std::path::PathBuf::from("/tmp"), None)
            .unwrap();

        manager
            .set_session_group(id, Some("backend".to_string()), Some("#FF0000".to_string()))
            .unwrap();

        let session = manager.get_session(id).unwrap();
        assert_eq!(session.group, Some("backend".to_string()));
        assert_eq!(session.color, Some("#FF0000".to_string()));
    }

    #[test]
    fn test_mock_session_manager_update_context_usage() {
        let manager = MockSessionManager {
            sessions: std::sync::Mutex::new(Vec::new()),
        };

        let id = manager
            .create_session("Test".to_string(), std::path::PathBuf::from("/tmp"), None)
            .unwrap();

        assert!(manager.get_session(id).unwrap().context_usage.is_none());

        manager.update_context_usage(id, Some(0.75));
        let session = manager.get_session(id).unwrap();
        assert!((session.context_usage.unwrap() - 0.75).abs() < f32::EPSILON);

        manager.update_context_usage(id, None);
        assert!(manager.get_session(id).unwrap().context_usage.is_none());

        manager.update_context_usage(SessionId(999), Some(0.5));
    }

    struct MockProcessMonitor {
        statuses: std::collections::HashMap<SessionId, SessionStatus>,
        patterns: Vec<String>,
    }

    impl ProcessMonitor for MockProcessMonitor {
        fn start_monitoring(&mut self, session_id: SessionId, _pty_pid: u32) -> Result<()> {
            self.statuses.insert(session_id, SessionStatus::Idle);
            Ok(())
        }

        fn stop_monitoring(&mut self, session_id: SessionId) {
            self.statuses.remove(&session_id);
        }

        fn get_status(&self, session_id: SessionId) -> Option<SessionStatus> {
            self.statuses.get(&session_id).copied()
        }

        fn add_pattern(&mut self, pattern: String) {
            self.patterns.push(pattern);
        }

        fn remove_pattern(&mut self, pattern: &str) {
            self.patterns.retain(|p| p != pattern);
        }
    }

    #[test]
    fn test_mock_process_monitor_start_stop() {
        let mut monitor = MockProcessMonitor {
            statuses: std::collections::HashMap::new(),
            patterns: Vec::new(),
        };

        monitor.start_monitoring(SessionId(1), 12345).unwrap();
        assert_eq!(monitor.get_status(SessionId(1)), Some(SessionStatus::Idle));

        monitor.stop_monitoring(SessionId(1));
        assert_eq!(monitor.get_status(SessionId(1)), None);
    }

    #[test]
    fn test_mock_process_monitor_patterns() {
        let mut monitor = MockProcessMonitor {
            statuses: std::collections::HashMap::new(),
            patterns: Vec::new(),
        };

        monitor.add_pattern("y/n".to_string());
        monitor.add_pattern("[Y/n]".to_string());
        assert_eq!(monitor.patterns.len(), 2);

        monitor.remove_pattern("y/n");
        assert_eq!(monitor.patterns.len(), 1);
        assert_eq!(monitor.patterns[0], "[Y/n]");
    }

    struct MockStorageService {
        dir: std::path::PathBuf,
    }

    impl StorageService for MockStorageService {
        fn codirigent_dir(&self) -> &Path {
            &self.dir
        }

        fn load_state(&self) -> Result<AppState> {
            Ok(AppState::default())
        }

        fn save_state(&self, _state: &AppState) -> Result<()> {
            Ok(())
        }

        fn load_task(&self, _id: &TaskId) -> Result<Option<Task>> {
            Ok(None)
        }

        fn save_task(&self, _task: &Task) -> Result<()> {
            Ok(())
        }

        fn list_task_ids(&self) -> Result<Vec<TaskId>> {
            Ok(Vec::new())
        }

        fn delete_task(&self, _id: &TaskId) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_mock_storage_service() {
        let storage = MockStorageService {
            dir: std::path::PathBuf::from("/tmp/.codirigent"),
        };

        assert_eq!(
            storage.codirigent_dir(),
            std::path::Path::new("/tmp/.codirigent")
        );

        let state = storage.load_state().unwrap();
        assert!(state.sessions.is_empty());

        storage.save_state(&state).unwrap();

        let task_ids = storage.list_task_ids().unwrap();
        assert!(task_ids.is_empty());
    }
}
