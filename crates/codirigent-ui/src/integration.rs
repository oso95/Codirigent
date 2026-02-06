//! Integration module for wiring all components together.
//!
//! This module provides the core application state management that
//! coordinates the session manager, input detector, storage service,
//! and event bus. It does not depend on GPUI, allowing it to be used
//! in headless or non-GUI contexts.
//!
//! # Architecture
//!
//! The integration layer connects:
//! - `SessionManager` - PTY spawning and session lifecycle
//! - `InputDetector` - Process monitoring and input detection
//! - `StorageService` - Persistent state storage
//! - `EventBus` - Cross-module communication
//!
//! # Example
//!
//! ```no_run
//! use codirigent_ui::integration::CodirigentIntegration;
//! use std::path::PathBuf;
//!
//! let integration = CodirigentIntegration::new(PathBuf::from(".")).unwrap();
//!
//! // Create a session
//! let session_id = integration.create_session(
//!     "My Session".to_string(),
//!     PathBuf::from("/tmp"),
//! ).unwrap();
//!
//! // The session is automatically monitored for input detection
//! ```

use anyhow::{anyhow, Context, Result};
use codirigent_core::{
    AppState, DefaultEventBus, CodirigentEvent, EventBus, FileStorageService, ProcessMonitor,
    Session, SessionId, SessionManager, SessionStatus, StorageService,
};
use codirigent_detector::{DetectorConfig, InputDetector};
use codirigent_session::DefaultSessionManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Configuration for the integration layer.
///
/// Controls behavior of the application integration including
/// auto-save settings and event processing options.
#[derive(Debug, Clone)]
pub struct IntegrationConfig {
    /// Whether to automatically save state when sessions change.
    pub auto_save_enabled: bool,
    /// Whether to start the event processing loop automatically.
    pub auto_start_event_loop: bool,
    /// Detector configuration for input monitoring.
    pub detector_config: DetectorConfig,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            auto_save_enabled: true,
            auto_start_event_loop: true,
            detector_config: DetectorConfig::default(),
        }
    }
}

/// Main integration layer connecting all Codirigent components.
///
/// This struct owns and coordinates all the core services:
/// - Session manager for PTY and session lifecycle
/// - Input detector for monitoring session status
/// - Storage service for persistence
/// - Event bus for cross-module communication
///
/// Thread-safe access is provided via internal mutexes.
pub struct CodirigentIntegration {
    /// Session manager for PTY and session lifecycle.
    session_manager: Arc<Mutex<DefaultSessionManager>>,
    /// Input detector for monitoring session status.
    detector: Arc<Mutex<InputDetector>>,
    /// Storage service for persistence.
    storage: Arc<FileStorageService>,
    /// Event bus for cross-module communication.
    event_bus: Arc<DefaultEventBus>,
    /// Configuration.
    config: IntegrationConfig,
}

impl CodirigentIntegration {
    /// Create a new integration layer with default configuration.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - The project directory where `.codirigent` will be created
    ///
    /// # Errors
    ///
    /// Returns an error if the storage service cannot be initialized.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use codirigent_ui::integration::CodirigentIntegration;
    /// use std::path::PathBuf;
    ///
    /// let integration = CodirigentIntegration::new(PathBuf::from(".")).unwrap();
    /// ```
    pub fn new(project_dir: PathBuf) -> Result<Self> {
        Self::with_config(project_dir, IntegrationConfig::default())
    }

    /// Create a new integration layer with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `project_dir` - The project directory where `.codirigent` will be created
    /// * `config` - Integration configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the storage service cannot be initialized.
    pub fn with_config(project_dir: PathBuf, config: IntegrationConfig) -> Result<Self> {
        info!(?project_dir, "Initializing Codirigent integration");

        let event_bus = Arc::new(DefaultEventBus::new(64));

        let storage = Arc::new(
            FileStorageService::new(&project_dir)
                .context("Failed to create storage service")?,
        );

        let session_manager = Arc::new(Mutex::new(
            DefaultSessionManager::new(event_bus.clone()),
        ));

        let detector = Arc::new(Mutex::new(InputDetector::new(
            config.detector_config.clone(),
            event_bus.clone(),
        )));

        let integration = Self {
            session_manager,
            detector,
            storage,
            event_bus,
            config,
        };

        if integration.config.auto_start_event_loop {
            integration.start_event_loop();
        }

        Ok(integration)
    }

    /// Start the event processing loop in a background thread.
    ///
    /// The loop processes events from the event bus and handles:
    /// - Session lifecycle events (created, closed)
    /// - Status change events
    /// - Input detection events
    ///
    /// Events are processed asynchronously to avoid blocking the main thread.
    pub fn start_event_loop(&self) {
        let event_bus = self.event_bus.clone();
        let session_manager = self.session_manager.clone();
        let storage = self.storage.clone();
        let auto_save = self.config.auto_save_enabled;

        thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!(error = %e, "Failed to create tokio runtime for event loop");
                    return;
                }
            };

            rt.block_on(async {
                let mut rx = event_bus.subscribe();

                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            Self::handle_event(&event, &session_manager, &storage, auto_save);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Event bus closed, stopping event loop");
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(count)) => {
                            warn!(count, "Event loop lagged, some events were missed");
                            continue;
                        }
                    }
                }
            });
        });
    }

    /// Handle a single event.
    fn handle_event(
        event: &CodirigentEvent,
        session_manager: &Arc<Mutex<DefaultSessionManager>>,
        storage: &Arc<FileStorageService>,
        auto_save: bool,
    ) {
        debug!(?event, "Handling event");

        match event {
            CodirigentEvent::SessionCreated { id } => {
                info!(%id, "Session created");
                if auto_save {
                    Self::save_state_internal(session_manager, storage);
                }
            }
            CodirigentEvent::SessionClosed { id } => {
                info!(%id, "Session closed");
                if auto_save {
                    Self::save_state_internal(session_manager, storage);
                }
            }
            CodirigentEvent::SessionStatusChanged { id, old, new } => {
                debug!(%id, ?old, ?new, "Session status changed");
            }
            CodirigentEvent::InputRequired { session_id, pattern } => {
                info!(%session_id, ?pattern, "Input required");
            }
            CodirigentEvent::InputProvided { session_id } => {
                debug!(%session_id, "Input provided");
            }
            CodirigentEvent::SessionRenamed { id, old_name, new_name } => {
                debug!(%id, %old_name, %new_name, "Session renamed");
                if auto_save {
                    Self::save_state_internal(session_manager, storage);
                }
            }
            CodirigentEvent::SessionGroupChanged { id, group, color } => {
                debug!(%id, ?group, ?color, "Session group changed");
                if auto_save {
                    Self::save_state_internal(session_manager, storage);
                }
            }
            _ => {
                debug!(?event, "Unhandled event type");
            }
        }
    }

    /// Save state internally (called from event handler).
    fn save_state_internal(
        session_manager: &Arc<Mutex<DefaultSessionManager>>,
        storage: &Arc<FileStorageService>,
    ) {
        if let Ok(manager) = session_manager.lock() {
            let sessions: Vec<Session> = manager.list_sessions();
            let state = AppState {
                sessions,
                layout: codirigent_core::LayoutMode::default(),
                updated_at: Some(chrono::Utc::now()),
            };

            if let Err(e) = storage.save_state(&state) {
                error!(error = %e, "Failed to save state");
            }
        } else {
            warn!("Failed to acquire lock for saving state");
        }
    }

    // --- Session Management ---

    /// Create a new session.
    ///
    /// Creates a PTY, starts the session, and begins monitoring for input.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable session name
    /// * `working_dir` - Working directory for the session
    ///
    /// # Returns
    ///
    /// The session ID of the newly created session.
    ///
    /// # Errors
    ///
    /// Returns an error if session creation or monitoring setup fails.
    pub fn create_session(&self, name: String, working_dir: PathBuf) -> Result<SessionId> {
        let session_id = {
            let manager = self.lock_session_manager()?;
            manager.create_session(name, working_dir)?
        };

        // Get child PID and start monitoring
        let child_pid = {
            let manager = self.lock_session_manager()?;
            manager.get_child_pid(session_id)
                .ok_or_else(|| anyhow!("Session created but child PID not available"))?
        };

        {
            let mut detector = self.lock_detector()?;
            detector.start_monitoring(session_id, child_pid)?;
        }

        Ok(session_id)
    }

    /// Close a session.
    ///
    /// Stops monitoring and closes the PTY for the session.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID to close
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist.
    pub fn close_session(&self, id: SessionId) -> Result<()> {
        // Stop monitoring first
        {
            let mut detector = self.lock_detector()?;
            detector.stop_monitoring(id);
        }

        // Close the session
        let manager = self.lock_session_manager()?;
        manager.close_session(id)?;

        Ok(())
    }

    /// Get a session by ID.
    pub fn get_session(&self, id: SessionId) -> Result<Option<Session>> {
        let manager = self.lock_session_manager()?;
        Ok(manager.get_session(id))
    }

    /// List all sessions.
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let manager = self.lock_session_manager()?;
        Ok(manager.list_sessions())
    }

    /// Send input to a session.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID
    /// * `input` - Input bytes to send
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist.
    pub fn send_input(&self, id: SessionId, input: &[u8]) -> Result<()> {
        let manager = self.lock_session_manager()?;
        manager.send_input(id, input)?;
        Ok(())
    }

    /// Resize a session's terminal.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID
    /// * `rows` - New row count
    /// * `cols` - New column count
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist.
    pub fn resize_session(&self, id: SessionId, rows: u16, cols: u16) -> Result<()> {
        let manager = self.lock_session_manager()?;
        manager.resize(id, rows, cols)?;
        Ok(())
    }

    /// Rename a session.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID
    /// * `new_name` - New name for the session
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist.
    pub fn rename_session(&self, id: SessionId, new_name: String) -> Result<()> {
        let manager = self.lock_session_manager()?;
        manager.rename_session(id, new_name)?;
        Ok(())
    }

    /// Set session group and color.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID
    /// * `group` - Optional group name
    /// * `color` - Optional color (hex string)
    ///
    /// # Errors
    ///
    /// Returns an error if the session doesn't exist.
    pub fn set_session_group(
        &self,
        id: SessionId,
        group: Option<String>,
        color: Option<String>,
    ) -> Result<()> {
        let manager = self.lock_session_manager()?;
        manager.set_session_group(id, group, color)?;
        Ok(())
    }

    /// Get session status from the detector.
    ///
    /// Returns the detected status (idle, working, waiting for input).
    pub fn get_session_status(&self, id: SessionId) -> Result<Option<SessionStatus>> {
        let detector = self.lock_detector()?;
        Ok(detector.get_status(id))
    }

    /// Process output from a session.
    ///
    /// This should be called when new output is received from a session's PTY.
    /// The detector will check for input patterns and update the status.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID
    /// * `data` - Output bytes from the PTY
    pub fn process_output(&self, id: SessionId, data: &[u8]) -> Result<()> {
        let mut detector = self.lock_detector()?;
        detector.process_output(id, data);
        Ok(())
    }

    /// Drain output from a session.
    ///
    /// Collects all available output from the session's PTY output channel.
    /// Returns `None` if no output is available.
    pub fn drain_output(&self, id: SessionId) -> Result<Option<Vec<u8>>> {
        let manager = self.lock_session_manager()?;
        Ok(manager.try_drain_output(id))
    }

    // --- State Persistence ---

    /// Save current state to disk.
    ///
    /// Persists all session metadata to the `.codirigent/state.json` file.
    pub fn save_state(&self) -> Result<()> {
        let sessions = self.list_sessions()?;
        let state = AppState {
            sessions,
            layout: codirigent_core::LayoutMode::default(),
            updated_at: Some(chrono::Utc::now()),
        };
        self.storage.save_state(&state)?;
        Ok(())
    }

    /// Load saved state from disk.
    ///
    /// Returns the persisted application state.
    pub fn load_state(&self) -> Result<AppState> {
        self.storage.load_state()
    }

    /// Restore sessions from saved state.
    ///
    /// Recreates sessions based on the persisted state. Note that PTY
    /// processes are not restored - new shells are spawned.
    ///
    /// # Returns
    ///
    /// The number of sessions restored.
    pub fn restore_sessions(&self) -> Result<usize> {
        let state = self.load_state()?;
        info!(session_count = state.sessions.len(), "Restoring sessions");

        let mut restored = 0;
        for session in state.sessions {
            match self.create_session(session.name.clone(), session.working_directory.clone()) {
                Ok(id) => {
                    // Restore group/color if present
                    if session.group.is_some() || session.color.is_some() {
                        if let Err(e) = self.set_session_group(id, session.group, session.color) {
                            warn!(error = %e, %id, "Failed to restore session group");
                        }
                    }
                    restored += 1;
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        session_name = %session.name,
                        "Failed to restore session"
                    );
                }
            }
        }

        info!(restored, "Sessions restored");
        Ok(restored)
    }

    // --- Event Bus Access ---

    /// Subscribe to events.
    ///
    /// Returns a receiver for receiving events from the event bus.
    pub fn subscribe(&self) -> broadcast::Receiver<CodirigentEvent> {
        self.event_bus.subscribe()
    }

    /// Publish an event.
    ///
    /// Sends an event to all subscribers.
    pub fn publish(&self, event: CodirigentEvent) {
        self.event_bus.publish(event);
    }

    // --- Accessors ---

    /// Get a reference to the event bus.
    pub fn event_bus(&self) -> &Arc<DefaultEventBus> {
        &self.event_bus
    }

    /// Get a reference to the storage service.
    pub fn storage(&self) -> &Arc<FileStorageService> {
        &self.storage
    }

    /// Get the number of active sessions.
    pub fn session_count(&self) -> Result<usize> {
        let manager = self.lock_session_manager()?;
        Ok(manager.session_count())
    }

    // --- Internal Helpers ---

    /// Lock the session manager.
    fn lock_session_manager(&self) -> Result<MutexGuard<'_, DefaultSessionManager>> {
        self.session_manager
            .lock()
            .map_err(|_| anyhow!("Session manager lock poisoned"))
    }

    /// Lock the detector.
    fn lock_detector(&self) -> Result<MutexGuard<'_, InputDetector>> {
        self.detector
            .lock()
            .map_err(|_| anyhow!("Detector lock poisoned"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_integration() -> (CodirigentIntegration, TempDir) {
        let temp = TempDir::new().unwrap();
        let config = IntegrationConfig {
            auto_start_event_loop: false,
            ..Default::default()
        };
        let integration = CodirigentIntegration::with_config(
            temp.path().to_path_buf(),
            config,
        ).unwrap();
        (integration, temp)
    }

    #[test]
    fn test_integration_new() {
        let (integration, _temp) = create_test_integration();
        assert_eq!(integration.session_count().unwrap(), 0);
    }

    #[test]
    fn test_integration_config_default() {
        let config = IntegrationConfig::default();
        assert!(config.auto_save_enabled);
        assert!(config.auto_start_event_loop);
    }

    #[test]
    fn test_integration_create_session() {
        let (integration, temp) = create_test_integration();

        let id = integration.create_session(
            "Test Session".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        assert_eq!(integration.session_count().unwrap(), 1);

        let session = integration.get_session(id).unwrap().unwrap();
        assert_eq!(session.name, "Test Session");
    }

    #[test]
    fn test_integration_close_session() {
        let (integration, temp) = create_test_integration();

        let id = integration.create_session(
            "Test Session".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        integration.close_session(id).unwrap();
        assert_eq!(integration.session_count().unwrap(), 0);
    }

    #[test]
    fn test_integration_list_sessions() {
        let (integration, temp) = create_test_integration();

        integration.create_session(
            "Session 1".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();
        integration.create_session(
            "Session 2".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        let sessions = integration.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_integration_send_input() {
        let (integration, temp) = create_test_integration();

        let id = integration.create_session(
            "Test Session".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        let result = integration.send_input(id, b"echo hello\n");
        assert!(result.is_ok());
    }

    #[test]
    fn test_integration_resize_session() {
        let (integration, temp) = create_test_integration();

        let id = integration.create_session(
            "Test Session".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        let result = integration.resize_session(id, 48, 120);
        assert!(result.is_ok());
    }

    #[test]
    fn test_integration_rename_session() {
        let (integration, temp) = create_test_integration();

        let id = integration.create_session(
            "Original".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        integration.rename_session(id, "Renamed".to_string()).unwrap();

        let session = integration.get_session(id).unwrap().unwrap();
        assert_eq!(session.name, "Renamed");
    }

    #[test]
    fn test_integration_set_session_group() {
        let (integration, temp) = create_test_integration();

        let id = integration.create_session(
            "Test Session".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        integration.set_session_group(
            id,
            Some("backend".to_string()),
            Some("#ff0000".to_string()),
        ).unwrap();

        let session = integration.get_session(id).unwrap().unwrap();
        assert_eq!(session.group, Some("backend".to_string()));
        assert_eq!(session.color, Some("#ff0000".to_string()));
    }

    #[test]
    fn test_integration_get_session_status() {
        let (integration, temp) = create_test_integration();

        let id = integration.create_session(
            "Test Session".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        let status = integration.get_session_status(id).unwrap();
        assert!(status.is_some());
    }

    #[test]
    fn test_integration_process_output() {
        let (integration, temp) = create_test_integration();

        let id = integration.create_session(
            "Test Session".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        let result = integration.process_output(id, b"Continue? [y/n]");
        assert!(result.is_ok());

        let status = integration.get_session_status(id).unwrap();
        assert_eq!(status, Some(SessionStatus::WaitingForInput));
    }

    #[test]
    fn test_integration_save_and_load_state() {
        let (integration, temp) = create_test_integration();

        integration.create_session(
            "Session 1".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        integration.save_state().unwrap();

        let state = integration.load_state().unwrap();
        assert_eq!(state.sessions.len(), 1);
        assert_eq!(state.sessions[0].name, "Session 1");
    }

    #[test]
    fn test_integration_restore_sessions() {
        let temp = TempDir::new().unwrap();

        // Create initial integration and sessions
        {
            let config = IntegrationConfig {
                auto_start_event_loop: false,
                ..Default::default()
            };
            let integration = CodirigentIntegration::with_config(
                temp.path().to_path_buf(),
                config,
            ).unwrap();

            integration.create_session(
                "Persistent Session".to_string(),
                temp.path().to_path_buf(),
            ).unwrap();

            integration.save_state().unwrap();
        }

        // Create new integration and restore
        {
            let config = IntegrationConfig {
                auto_start_event_loop: false,
                ..Default::default()
            };
            let integration = CodirigentIntegration::with_config(
                temp.path().to_path_buf(),
                config,
            ).unwrap();

            let restored = integration.restore_sessions().unwrap();
            assert_eq!(restored, 1);
            assert_eq!(integration.session_count().unwrap(), 1);
        }
    }

    #[test]
    fn test_integration_subscribe() {
        let (integration, _temp) = create_test_integration();

        let mut rx = integration.subscribe();

        integration.publish(CodirigentEvent::SessionCreated { id: SessionId(1) });

        // Try to receive the event (non-blocking)
        let event = rx.try_recv();
        assert!(event.is_ok());
    }

    #[test]
    fn test_integration_event_bus_accessor() {
        let (integration, _temp) = create_test_integration();
        let event_bus = integration.event_bus();
        assert!(Arc::strong_count(event_bus) >= 1);
    }

    #[test]
    fn test_integration_storage_accessor() {
        let (integration, _temp) = create_test_integration();
        let storage = integration.storage();
        assert!(Arc::strong_count(storage) >= 1);
    }

    #[test]
    fn test_integration_close_nonexistent_session() {
        let (integration, _temp) = create_test_integration();
        let result = integration.close_session(SessionId(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_get_nonexistent_session() {
        let (integration, _temp) = create_test_integration();
        let session = integration.get_session(SessionId(999)).unwrap();
        assert!(session.is_none());
    }

    #[test]
    fn test_integration_send_input_nonexistent() {
        let (integration, _temp) = create_test_integration();
        let result = integration.send_input(SessionId(999), b"test");
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_resize_nonexistent() {
        let (integration, _temp) = create_test_integration();
        let result = integration.resize_session(SessionId(999), 24, 80);
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_rename_nonexistent() {
        let (integration, _temp) = create_test_integration();
        let result = integration.rename_session(SessionId(999), "New Name".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_set_group_nonexistent() {
        let (integration, _temp) = create_test_integration();
        let result = integration.set_session_group(SessionId(999), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_integration_get_status_nonexistent() {
        let (integration, _temp) = create_test_integration();
        let status = integration.get_session_status(SessionId(999)).unwrap();
        assert!(status.is_none());
    }

    #[test]
    fn test_integration_restore_with_group() {
        let temp = TempDir::new().unwrap();

        // Create initial integration with grouped session
        {
            let config = IntegrationConfig {
                auto_start_event_loop: false,
                ..Default::default()
            };
            let integration = CodirigentIntegration::with_config(
                temp.path().to_path_buf(),
                config,
            ).unwrap();

            let id = integration.create_session(
                "Grouped Session".to_string(),
                temp.path().to_path_buf(),
            ).unwrap();

            integration.set_session_group(
                id,
                Some("backend".to_string()),
                Some("#00ff00".to_string()),
            ).unwrap();

            integration.save_state().unwrap();
        }

        // Create new integration and restore
        {
            let config = IntegrationConfig {
                auto_start_event_loop: false,
                ..Default::default()
            };
            let integration = CodirigentIntegration::with_config(
                temp.path().to_path_buf(),
                config,
            ).unwrap();

            let restored = integration.restore_sessions().unwrap();
            assert_eq!(restored, 1);

            let sessions = integration.list_sessions().unwrap();
            // Note: group/color may not be restored since sessions are recreated
            // This test verifies the restore process doesn't fail
            assert_eq!(sessions.len(), 1);
        }
    }

    #[test]
    fn test_integration_multiple_sessions_lifecycle() {
        let (integration, temp) = create_test_integration();

        // Create multiple sessions
        let id1 = integration.create_session(
            "Session 1".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();
        let id2 = integration.create_session(
            "Session 2".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();
        let id3 = integration.create_session(
            "Session 3".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        assert_eq!(integration.session_count().unwrap(), 3);

        // Close one session
        integration.close_session(id2).unwrap();
        assert_eq!(integration.session_count().unwrap(), 2);

        // Verify remaining sessions
        assert!(integration.get_session(id1).unwrap().is_some());
        assert!(integration.get_session(id2).unwrap().is_none());
        assert!(integration.get_session(id3).unwrap().is_some());
    }

    #[test]
    fn test_handle_event_session_created() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(
            DefaultSessionManager::new(event_bus.clone()),
        ));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::SessionCreated { id: SessionId(1) };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_session_closed() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(
            DefaultSessionManager::new(event_bus.clone()),
        ));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::SessionClosed { id: SessionId(1) };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_status_changed() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(
            DefaultSessionManager::new(event_bus.clone()),
        ));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::SessionStatusChanged {
            id: SessionId(1),
            old: SessionStatus::Idle,
            new: SessionStatus::Working,
        };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_input_required() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(
            DefaultSessionManager::new(event_bus.clone()),
        ));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::InputRequired {
            session_id: SessionId(1),
            pattern: Some("[y/n]".to_string()),
        };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_input_provided() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(
            DefaultSessionManager::new(event_bus.clone()),
        ));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::InputProvided { session_id: SessionId(1) };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_session_renamed() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(
            DefaultSessionManager::new(event_bus.clone()),
        ));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::SessionRenamed {
            id: SessionId(1),
            old_name: "Old".to_string(),
            new_name: "New".to_string(),
        };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_group_changed() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(
            DefaultSessionManager::new(event_bus.clone()),
        ));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::SessionGroupChanged {
            id: SessionId(1),
            group: Some("backend".to_string()),
            color: Some("#ff0000".to_string()),
        };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, false);
        // Should not panic
    }

    #[test]
    fn test_drain_output() {
        let (integration, temp) = create_test_integration();

        let id = integration.create_session(
            "Test Session".to_string(),
            temp.path().to_path_buf(),
        ).unwrap();

        // Drain should work even if empty
        let output = integration.drain_output(id).unwrap();
        // May or may not have output depending on shell initialization
        let _ = output;
    }

    #[test]
    fn test_drain_output_nonexistent() {
        let (integration, _temp) = create_test_integration();
        let output = integration.drain_output(SessionId(999)).unwrap();
        assert!(output.is_none());
    }

    #[test]
    fn test_handle_context_usage_updated_event() {
        let (integration, _temp) = create_test_integration();
        let event = CodirigentEvent::ContextUsageUpdated {
            session_id: SessionId(1),
            percentage: 0.65,
            effective_percentage: 0.72,
        };
        // Should not panic
        CodirigentIntegration::handle_event(
            &event,
            &integration.session_manager,
            &integration.storage,
            false,
        );
    }

    #[test]
    fn test_handle_context_threshold_reached_event() {
        let (integration, _temp) = create_test_integration();
        let event = CodirigentEvent::ContextThresholdReached {
            session_id: SessionId(1),
            threshold: 0.7,
            state: codirigent_core::ContextThresholdState::Warning,
        };
        // Should not panic
        CodirigentIntegration::handle_event(
            &event,
            &integration.session_manager,
            &integration.storage,
            false,
        );
    }
}
