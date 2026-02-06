//! Session state management.
//!
//! This module provides the internal session state representation
//! that combines session metadata with runtime PTY handles.

use crate::pty::PtyHandle;
use codirigent_core::{Session, SessionId, SessionStatus};
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Internal session state combining metadata with runtime handles.
///
/// This struct holds both the persistent session metadata and the
/// runtime resources needed for terminal I/O. It is used internally
/// by the session manager and not exposed directly to other crates.
pub struct SessionState {
    /// Session metadata (persisted).
    pub session: Session,
    /// PTY handle for terminal I/O.
    pub pty: PtyHandle,
    /// Channel receiving PTY output.
    pub output_rx: mpsc::Receiver<Vec<u8>>,
    /// Path to the context file for file-based IPC with CLI hooks.
    pub context_file_path: Option<PathBuf>,
}

impl SessionState {
    /// Create a new session state.
    ///
    /// # Arguments
    ///
    /// * `session` - The session metadata
    /// * `pty` - The PTY handle for terminal I/O
    /// * `output_rx` - Channel for receiving PTY output
    pub fn new(session: Session, pty: PtyHandle, output_rx: mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            session,
            pty,
            output_rx,
            context_file_path: None,
        }
    }

    /// Create a new session state with a context file path.
    pub fn with_context_file(mut self, path: PathBuf) -> Self {
        self.context_file_path = Some(path);
        self
    }

    /// Get session ID.
    pub fn id(&self) -> SessionId {
        self.session.id
    }

    /// Get current status.
    pub fn status(&self) -> SessionStatus {
        self.session.status
    }

    /// Update status.
    pub fn set_status(&mut self, status: SessionStatus) {
        self.session.status = status;
    }

    /// Get session name.
    pub fn name(&self) -> &str {
        &self.session.name
    }

    /// Get working directory.
    pub fn working_directory(&self) -> &std::path::Path {
        &self.session.working_directory
    }

    /// Get the PTY child process ID.
    pub fn child_pid(&self) -> u32 {
        self.pty.child_pid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::{spawn_output_reader, PtyHandle};
    use tempfile::TempDir;

    fn create_test_session_state() -> (SessionState, TempDir) {
        let temp = TempDir::new().unwrap();
        let mut pty = PtyHandle::spawn(temp.path(), 24, 80, &[]).unwrap();
        let reader = pty.take_reader().unwrap();
        let output_rx = spawn_output_reader(reader);

        let session = Session::new(
            SessionId(1),
            "Test Session".to_string(),
            temp.path().to_path_buf(),
        );

        (SessionState::new(session, pty, output_rx), temp)
    }

    #[test]
    fn test_session_state_new() {
        let (state, _temp) = create_test_session_state();

        assert_eq!(state.id(), SessionId(1));
        assert_eq!(state.name(), "Test Session");
        assert_eq!(state.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_session_state_id() {
        let (state, _temp) = create_test_session_state();
        assert_eq!(state.id(), SessionId(1));
    }

    #[test]
    fn test_session_state_status() {
        let (state, _temp) = create_test_session_state();
        assert_eq!(state.status(), SessionStatus::Idle);
    }

    #[test]
    fn test_session_state_set_status() {
        let (mut state, _temp) = create_test_session_state();

        state.set_status(SessionStatus::Working);
        assert_eq!(state.status(), SessionStatus::Working);

        state.set_status(SessionStatus::WaitingForInput);
        assert_eq!(state.status(), SessionStatus::WaitingForInput);
    }

    #[test]
    fn test_session_state_name() {
        let (state, _temp) = create_test_session_state();
        assert_eq!(state.name(), "Test Session");
    }

    #[test]
    fn test_session_state_working_directory() {
        let (state, temp) = create_test_session_state();
        assert_eq!(state.working_directory(), temp.path());
    }

    #[test]
    fn test_session_state_child_pid() {
        let (state, _temp) = create_test_session_state();
        assert!(state.child_pid() > 0);
    }

    #[test]
    fn test_session_state_access_session_fields() {
        let (state, _temp) = create_test_session_state();

        // Access session metadata directly
        assert_eq!(state.session.id, SessionId(1));
        assert_eq!(state.session.name, "Test Session");
        assert!(state.session.current_task.is_none());
        assert!(state.session.group.is_none());
        assert!(state.session.color.is_none());
    }

    #[test]
    fn test_session_state_modify_session() {
        let (mut state, _temp) = create_test_session_state();

        state.session.name = "Renamed Session".to_string();
        assert_eq!(state.name(), "Renamed Session");

        state.session.group = Some("my-group".to_string());
        assert_eq!(state.session.group, Some("my-group".to_string()));
    }
}
