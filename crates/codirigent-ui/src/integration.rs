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
//!     None,
//! ).unwrap();
//!
//! // The session is automatically monitored for input detection
//! ```

use anyhow::{anyhow, Context, Result};
use codirigent_core::{
    AppState, CodirigentEvent, CompactionConfig, CompactionService, DefaultEventBus, EventBus,
    FileStorageService, ProcessMonitor, Session, SessionId, SessionManager, SessionStatus,
    StorageService, TaskManager, TaskManagerConfig,
};
use codirigent_detector::{notify_input_required, DetectorConfig, InputDetector};
use codirigent_session::DefaultSessionManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;
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
    /// Configuration for auto-compaction before verification.
    pub compaction: CompactionConfig,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            auto_save_enabled: true,
            auto_start_event_loop: true,
            detector_config: DetectorConfig::default(),
            compaction: CompactionConfig::default(),
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
    /// Compaction service for auto-compacting before verification.
    compaction: Arc<Mutex<CompactionService>>,
    /// Task manager for unified task management.
    task_manager: Arc<Mutex<TaskManager>>,
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
            FileStorageService::new(&project_dir).context("Failed to create storage service")?,
        );

        let session_manager = Arc::new(Mutex::new(DefaultSessionManager::new(event_bus.clone())));

        let detector = Arc::new(Mutex::new(InputDetector::new(
            config.detector_config.clone(),
            event_bus.clone(),
        )));

        let compaction = Arc::new(Mutex::new(CompactionService::new(config.compaction.clone())));

        let task_manager = Arc::new(Mutex::new(TaskManager::new(
            TaskManagerConfig::default(),
            storage.clone(),
            event_bus.clone(),
        )));

        let integration = Self {
            session_manager,
            detector,
            storage,
            event_bus,
            compaction,
            task_manager,
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
        let task_manager = self.task_manager.clone();
        let auto_save = self.config.auto_save_enabled;
        let notifications_enabled = self.config.detector_config.notifications_enabled;

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
                            Self::handle_event(
                            &event,
                            &session_manager,
                            &storage,
                            &task_manager,
                            auto_save,
                            notifications_enabled,
                        );
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
        task_manager: &Arc<Mutex<TaskManager>>,
        auto_save: bool,
        notifications_enabled: bool,
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

                // Check if session had an active task and mark it as blocked
                if let Ok(mut task_mgr) = task_manager.lock() {
                    if let Some((task_id, task)) = task_mgr.find_task_by_session(*id) {
                        if matches!(
                            task.status,
                            codirigent_core::TaskStatus::Working | codirigent_core::TaskStatus::Verifying
                        ) {
                            warn!(%id, ?task_id, "Session closed with active task, marking as blocked");
                            if let Err(e) = task_mgr.transition_task_status(
                                &task_id,
                                codirigent_core::TaskStatus::Blocked,
                                Some("Session closed unexpectedly".to_string()),
                            ) {
                                error!(error = %e, "Failed to block task after session close");
                            }
                        }
                    }
                }

                if auto_save {
                    Self::save_state_internal(session_manager, storage);
                }
            }
            CodirigentEvent::SessionStatusChanged { id, old, new } => {
                debug!(%id, ?old, ?new, "Session status changed");

                // Automatically sync task status based on session status
                if let Ok(mut task_mgr) = task_manager.lock() {
                    if let Some(updated_task_id) = task_mgr.on_session_status_changed(*id, *old, *new) {
                        info!(%id, ?updated_task_id, "Task status automatically synced with session");
                    }
                }
            }
            CodirigentEvent::InputRequired {
                session_id,
                pattern,
            } => {
                info!(%session_id, ?pattern, "Input required");
                if notifications_enabled {
                    let session_name = session_manager
                        .lock()
                        .ok()
                        .and_then(|mgr| mgr.get_session(*session_id))
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| format!("Session {}", session_id.0));
                    notify_input_required(*session_id, &session_name);
                }
            }
            CodirigentEvent::InputProvided { session_id } => {
                debug!(%session_id, "Input provided");
            }
            CodirigentEvent::SessionRenamed {
                id,
                old_name,
                new_name,
            } => {
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
    pub fn create_session(&self, name: String, working_dir: PathBuf, shell: Option<String>) -> Result<SessionId> {
        let session_id = {
            let manager = self.lock_session_manager()?;
            manager.create_session(name, working_dir, shell)?
        };

        // Get child PID and start monitoring
        let child_pid = {
            let manager = self.lock_session_manager()?;
            manager
                .get_child_pid(session_id)
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
            match self.create_session(session.name.clone(), session.working_directory.clone(), None) {
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

    // --- Compaction ---

    /// Check if a session is currently being compacted.
    ///
    /// Use this to guard idle handlers from starting verification or
    /// assigning new work while compaction is in progress.
    pub fn is_compacting(&self, session_id: SessionId) -> bool {
        self.compaction
            .lock()
            .map(|svc| svc.is_compacting(session_id))
            .unwrap_or(false)
    }

    /// Attempt to compact a session before verification.
    ///
    /// Checks context usage, and if above threshold, sends `/compact` via
    /// PTY stdin and spawns a background thread to wait for completion.
    ///
    /// Returns `true` if compaction was started, `false` if skipped.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session to compact
    /// * `context_usage` - The session's current context usage (0.0-1.0)
    pub fn try_compact_session(
        &self,
        session_id: SessionId,
        context_usage: Option<f32>,
    ) -> bool {
        let (should_compact, command, timeout_secs, focus) = {
            let mut compaction = match self.compaction.lock() {
                Ok(c) => c,
                Err(_) => {
                    warn!(%session_id, "Compaction lock poisoned, skipping compaction");
                    return false;
                }
            };

            if !compaction.should_compact(session_id, context_usage) {
                debug!(%session_id, ?context_usage, "Skipping compaction: conditions not met");
                return false;
            }

            if !compaction.begin_compaction(session_id) {
                debug!(%session_id, "Skipping compaction: already compacting");
                return false;
            }

            let command = compaction.compact_command();
            let timeout = compaction.timeout_secs();
            let focus = compaction.config().focus_instructions.clone();
            (true, command, timeout, focus)
        };

        if !should_compact {
            return false;
        }

        // Send /compact command via PTY stdin
        if let Err(e) = self.send_input(session_id, command.as_bytes()) {
            warn!(%session_id, error = %e, "Failed to send /compact command");
            if let Ok(mut compaction) = self.compaction.lock() {
                compaction.end_compaction(session_id);
            }
            return false;
        }

        info!(%session_id, ?focus, "Compaction started");

        // Publish CompactionStarted event
        self.event_bus.publish(CodirigentEvent::CompactionStarted {
            session_id,
            focus,
        });

        // Spawn background thread to wait for compaction to complete
        let event_bus = self.event_bus.clone();
        let compaction = self.compaction.clone();

        thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!(error = %e, "Failed to create tokio runtime for compaction waiter");
                    if let Ok(mut svc) = compaction.lock() {
                        svc.end_compaction(session_id);
                    }
                    return;
                }
            };

            rt.block_on(async {
                let mut rx = event_bus.subscribe();
                let timeout = Duration::from_secs(timeout_secs);

                let result = tokio::time::timeout(timeout, async {
                    loop {
                        match rx.recv().await {
                            Ok(CodirigentEvent::SessionStatusChanged {
                                id,
                                new: SessionStatus::Idle,
                                ..
                            }) if id == session_id => {
                                // Compaction complete (session returned to Idle)
                                return true;
                            }
                            Ok(CodirigentEvent::SessionStatusChanged {
                                id,
                                new: SessionStatus::Error,
                                ..
                            }) if id == session_id => {
                                // Error during compaction
                                return false;
                            }
                            Ok(CodirigentEvent::SessionStatusChanged {
                                id,
                                new: SessionStatus::WaitingForInput,
                                ..
                            }) if id == session_id => {
                                // Needs input during compaction - treat as failure
                                return false;
                            }
                            Ok(CodirigentEvent::SessionClosed { id })
                                if id == session_id =>
                            {
                                // Session closed during compaction
                                if let Ok(mut svc) = compaction.lock() {
                                    svc.end_compaction(session_id);
                                }
                                return false;
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                return false;
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                continue;
                            }
                            _ => continue,
                        }
                    }
                })
                .await;

                let success = match result {
                    Ok(success) => success,
                    Err(_) => {
                        warn!(%session_id, "Compaction timed out");
                        false
                    }
                };

                if let Ok(mut svc) = compaction.lock() {
                    svc.end_compaction(session_id);
                }

                if success {
                    info!(%session_id, "Compaction completed successfully");
                } else {
                    warn!(%session_id, "Compaction completed with failure");
                }

                event_bus.publish(CodirigentEvent::CompactionCompleted {
                    session_id,
                    success,
                });
            });
        });

        true
    }

    /// Get a reference to the compaction service.
    pub fn compaction(&self) -> &Arc<Mutex<CompactionService>> {
        &self.compaction
    }

    /// Get a reference to the task manager.
    pub fn task_manager(&self) -> &Arc<Mutex<TaskManager>> {
        &self.task_manager
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
        let integration =
            CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();
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

        let id = integration
            .create_session("Test Session".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        assert_eq!(integration.session_count().unwrap(), 1);

        let session = integration.get_session(id).unwrap().unwrap();
        assert_eq!(session.name, "Test Session");
    }

    #[test]
    fn test_integration_close_session() {
        let (integration, temp) = create_test_integration();

        let id = integration
            .create_session("Test Session".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        integration.close_session(id).unwrap();
        assert_eq!(integration.session_count().unwrap(), 0);
    }

    #[test]
    fn test_integration_list_sessions() {
        let (integration, temp) = create_test_integration();

        integration
            .create_session("Session 1".to_string(), temp.path().to_path_buf(), None)
            .unwrap();
        integration
            .create_session("Session 2".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        let sessions = integration.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_integration_send_input() {
        let (integration, temp) = create_test_integration();

        let id = integration
            .create_session("Test Session".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        let result = integration.send_input(id, b"echo hello\n");
        assert!(result.is_ok());
    }

    #[test]
    fn test_integration_resize_session() {
        let (integration, temp) = create_test_integration();

        let id = integration
            .create_session("Test Session".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        let result = integration.resize_session(id, 48, 120);
        assert!(result.is_ok());
    }

    #[test]
    fn test_integration_rename_session() {
        let (integration, temp) = create_test_integration();

        let id = integration
            .create_session("Original".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        integration
            .rename_session(id, "Renamed".to_string())
            .unwrap();

        let session = integration.get_session(id).unwrap().unwrap();
        assert_eq!(session.name, "Renamed");
    }

    #[test]
    fn test_integration_set_session_group() {
        let (integration, temp) = create_test_integration();

        let id = integration
            .create_session("Test Session".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        integration
            .set_session_group(id, Some("backend".to_string()), Some("#ff0000".to_string()))
            .unwrap();

        let session = integration.get_session(id).unwrap().unwrap();
        assert_eq!(session.group, Some("backend".to_string()));
        assert_eq!(session.color, Some("#ff0000".to_string()));
    }

    #[test]
    fn test_integration_get_session_status() {
        let (integration, temp) = create_test_integration();

        let id = integration
            .create_session("Test Session".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        let status = integration.get_session_status(id).unwrap();
        assert!(status.is_some());
    }

    #[test]
    fn test_integration_process_output() {
        let (integration, temp) = create_test_integration();

        let id = integration
            .create_session("Test Session".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        let result = integration.process_output(id, b"Continue? [y/n]");
        assert!(result.is_ok());

        let status = integration.get_session_status(id).unwrap();
        assert_eq!(status, Some(SessionStatus::WaitingForInput));
    }

    #[test]
    fn test_integration_save_and_load_state() {
        let (integration, temp) = create_test_integration();

        integration
            .create_session("Session 1".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

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
            let integration =
                CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

            integration
                .create_session("Persistent Session".to_string(), temp.path().to_path_buf(), None)
                .unwrap();

            integration.save_state().unwrap();
        }

        // Create new integration and restore
        {
            let config = IntegrationConfig {
                auto_start_event_loop: false,
                ..Default::default()
            };
            let integration =
                CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

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
            let integration =
                CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

            let id = integration
                .create_session("Grouped Session".to_string(), temp.path().to_path_buf(), None)
                .unwrap();

            integration
                .set_session_group(id, Some("backend".to_string()), Some("#00ff00".to_string()))
                .unwrap();

            integration.save_state().unwrap();
        }

        // Create new integration and restore
        {
            let config = IntegrationConfig {
                auto_start_event_loop: false,
                ..Default::default()
            };
            let integration =
                CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

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
        let id1 = integration
            .create_session("Session 1".to_string(), temp.path().to_path_buf(), None)
            .unwrap();
        let id2 = integration
            .create_session("Session 2".to_string(), temp.path().to_path_buf(), None)
            .unwrap();
        let id3 = integration
            .create_session("Session 3".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

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
        let session_manager = Arc::new(Mutex::new(DefaultSessionManager::new(event_bus.clone())));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::SessionCreated { id: SessionId(1) };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, &Arc::new(Mutex::new(TaskManager::new(TaskManagerConfig::default(), storage.clone(), event_bus.clone()))), false, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_session_closed() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(DefaultSessionManager::new(event_bus.clone())));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::SessionClosed { id: SessionId(1) };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, &Arc::new(Mutex::new(TaskManager::new(TaskManagerConfig::default(), storage.clone(), event_bus.clone()))), false, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_status_changed() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(DefaultSessionManager::new(event_bus.clone())));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::SessionStatusChanged {
            id: SessionId(1),
            old: SessionStatus::Idle,
            new: SessionStatus::Working,
        };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, &Arc::new(Mutex::new(TaskManager::new(TaskManagerConfig::default(), storage.clone(), event_bus.clone()))), false, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_input_required() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(DefaultSessionManager::new(event_bus.clone())));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::InputRequired {
            session_id: SessionId(1),
            pattern: Some("[y/n]".to_string()),
        };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, &Arc::new(Mutex::new(TaskManager::new(TaskManagerConfig::default(), storage.clone(), event_bus.clone()))), false, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_input_provided() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(DefaultSessionManager::new(event_bus.clone())));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::InputProvided {
            session_id: SessionId(1),
        };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, &Arc::new(Mutex::new(TaskManager::new(TaskManagerConfig::default(), storage.clone(), event_bus.clone()))), false, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_session_renamed() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(DefaultSessionManager::new(event_bus.clone())));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::SessionRenamed {
            id: SessionId(1),
            old_name: "Old".to_string(),
            new_name: "New".to_string(),
        };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, &Arc::new(Mutex::new(TaskManager::new(TaskManagerConfig::default(), storage.clone(), event_bus.clone()))), false, false);
        // Should not panic
    }

    #[test]
    fn test_handle_event_group_changed() {
        let temp = TempDir::new().unwrap();
        let event_bus = Arc::new(DefaultEventBus::new(16));
        let session_manager = Arc::new(Mutex::new(DefaultSessionManager::new(event_bus.clone())));
        let storage = Arc::new(FileStorageService::new(temp.path()).unwrap());

        let event = CodirigentEvent::SessionGroupChanged {
            id: SessionId(1),
            group: Some("backend".to_string()),
            color: Some("#ff0000".to_string()),
        };
        CodirigentIntegration::handle_event(&event, &session_manager, &storage, &Arc::new(Mutex::new(TaskManager::new(TaskManagerConfig::default(), storage.clone(), event_bus.clone()))), false, false);
        // Should not panic
    }

    #[test]
    fn test_drain_output() {
        let (integration, temp) = create_test_integration();

        let id = integration
            .create_session("Test Session".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

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
            &integration.task_manager,
            false,
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
            &integration.task_manager,
            false,
            false,
        );
    }

    // === Compaction Tests ===

    #[test]
    fn test_is_compacting_initially_false() {
        let (integration, _temp) = create_test_integration();
        assert!(!integration.is_compacting(SessionId(1)));
    }

    #[test]
    fn test_try_compact_session_skips_when_below_threshold() {
        let (integration, _temp) = create_test_integration();
        let result = integration.try_compact_session(SessionId(1), Some(0.1));
        assert!(!result);
        assert!(!integration.is_compacting(SessionId(1)));
    }

    #[test]
    fn test_try_compact_session_skips_when_context_unknown() {
        let (integration, _temp) = create_test_integration();
        let result = integration.try_compact_session(SessionId(1), None);
        assert!(!result);
        assert!(!integration.is_compacting(SessionId(1)));
    }

    #[test]
    fn test_try_compact_session_skips_nonexistent_session() {
        let (integration, _temp) = create_test_integration();
        // Session 999 doesn't exist, so send_input will fail and compaction
        // will clean up
        let result = integration.try_compact_session(SessionId(999), Some(0.8));
        assert!(!result);
        assert!(!integration.is_compacting(SessionId(999)));
    }

    #[test]
    fn test_try_compact_real_session() {
        let (integration, temp) = create_test_integration();

        let id = integration
            .create_session("Compact Test".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        // Try to compact with context above threshold
        let result = integration.try_compact_session(id, Some(0.8));
        assert!(result);
        assert!(integration.is_compacting(id));
    }

    #[test]
    fn test_try_compact_session_reentrancy_guard() {
        let (integration, temp) = create_test_integration();

        let id = integration
            .create_session("Guard Test".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        // First compact should succeed
        let result1 = integration.try_compact_session(id, Some(0.8));
        assert!(result1);

        // Second compact on same session should be skipped (re-entrancy guard)
        let result2 = integration.try_compact_session(id, Some(0.8));
        assert!(!result2);
    }

    #[test]
    fn test_compaction_disabled() {
        let temp = TempDir::new().unwrap();
        let config = IntegrationConfig {
            auto_start_event_loop: false,
            compaction: codirigent_core::CompactionConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let integration =
            CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

        let id = integration
            .create_session("Disabled Test".to_string(), temp.path().to_path_buf(), None)
            .unwrap();

        let result = integration.try_compact_session(id, Some(0.9));
        assert!(!result);
    }

    #[test]
    fn test_compaction_accessor() {
        let (integration, _temp) = create_test_integration();
        let compaction = integration.compaction();
        assert!(Arc::strong_count(compaction) >= 1);
    }

    #[test]
    fn test_handle_compaction_events() {
        let (integration, _temp) = create_test_integration();

        // CompactionStarted event should not panic
        let event = CodirigentEvent::CompactionStarted {
            session_id: SessionId(1),
            focus: Some("test".to_string()),
        };
        CodirigentIntegration::handle_event(
            &event,
            &integration.session_manager,
            &integration.storage,
            &integration.task_manager,
            false,
            false,
        );

        // CompactionCompleted event should not panic
        let event = CodirigentEvent::CompactionCompleted {
            session_id: SessionId(1),
            success: true,
        };
        CodirigentIntegration::handle_event(
            &event,
            &integration.session_manager,
            &integration.storage,
            &integration.task_manager,
            false,
            false,
        );
    }
}
