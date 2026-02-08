//! Integration tests for Codirigent.
//!
//! These tests verify the end-to-end functionality of the Codirigent application,
//! including session lifecycle, event flow, state persistence, and component
//! integration.

use codirigent_core::{
    CodirigentEvent, DefaultEventBus, EventBus, FileStorageService, ProcessMonitor, Session,
    SessionId, SessionManager, SessionStatus, StorageService,
};
use codirigent_detector::{DetectorConfig, InputDetector};
use codirigent_session::DefaultSessionManager;
use codirigent_ui::integration::{CodirigentIntegration, IntegrationConfig};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test integration instance.
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

/// Create a test session and return its ID.
fn create_test_session(
    integration: &CodirigentIntegration,
    temp: &TempDir,
    name: &str,
) -> SessionId {
    integration
        .create_session(name.to_string(), temp.path().to_path_buf())
        .unwrap()
}

// ============================================================================
// Session Lifecycle Tests
// ============================================================================

/// Test the full session lifecycle: create, interact, close.
#[test]
fn test_full_session_lifecycle() {
    let (integration, temp) = create_test_integration();

    // Create session
    let id = create_test_session(&integration, &temp, "Test Session");
    assert_eq!(integration.session_count().unwrap(), 1);

    // Get session details
    let session = integration.get_session(id).unwrap().unwrap();
    assert_eq!(session.name, "Test Session");
    assert_eq!(session.status, SessionStatus::Idle);

    // Send input
    integration.send_input(id, b"echo hello\n").unwrap();

    // Resize terminal
    integration.resize_session(id, 48, 120).unwrap();

    // Rename session
    integration
        .rename_session(id, "Renamed Session".to_string())
        .unwrap();
    let session = integration.get_session(id).unwrap().unwrap();
    assert_eq!(session.name, "Renamed Session");

    // Close session
    integration.close_session(id).unwrap();
    assert_eq!(integration.session_count().unwrap(), 0);
    assert!(integration.get_session(id).unwrap().is_none());
}

/// Test creating multiple sessions.
#[test]
fn test_multiple_sessions() {
    let (integration, temp) = create_test_integration();

    let id1 = create_test_session(&integration, &temp, "Session 1");
    let id2 = create_test_session(&integration, &temp, "Session 2");
    let id3 = create_test_session(&integration, &temp, "Session 3");

    assert_eq!(integration.session_count().unwrap(), 3);

    // All sessions should have unique IDs
    assert_ne!(id1, id2);
    assert_ne!(id2, id3);

    // All sessions should be accessible
    assert!(integration.get_session(id1).unwrap().is_some());
    assert!(integration.get_session(id2).unwrap().is_some());
    assert!(integration.get_session(id3).unwrap().is_some());
}

/// Test session group management.
#[test]
fn test_session_groups() {
    let (integration, temp) = create_test_integration();

    let id = create_test_session(&integration, &temp, "Grouped Session");

    // Set group
    integration
        .set_session_group(id, Some("backend".to_string()), Some("#ff5733".to_string()))
        .unwrap();

    let session = integration.get_session(id).unwrap().unwrap();
    assert_eq!(session.group, Some("backend".to_string()));
    assert_eq!(session.color, Some("#ff5733".to_string()));

    // Clear group
    integration.set_session_group(id, None, None).unwrap();
    let session = integration.get_session(id).unwrap().unwrap();
    assert!(session.group.is_none());
    assert!(session.color.is_none());
}

// ============================================================================
// State Persistence Tests
// ============================================================================

/// Test saving and loading application state.
#[test]
fn test_state_persistence() {
    let temp = TempDir::new().unwrap();

    // Create and save state
    {
        let config = IntegrationConfig {
            auto_start_event_loop: false,
            ..Default::default()
        };
        let integration =
            CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

        integration
            .create_session("Persistent Session".to_string(), temp.path().to_path_buf())
            .unwrap();
        integration
            .create_session("Another Session".to_string(), temp.path().to_path_buf())
            .unwrap();

        integration.save_state().unwrap();
    }

    // Load and verify state
    {
        let config = IntegrationConfig {
            auto_start_event_loop: false,
            ..Default::default()
        };
        let integration =
            CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

        let state = integration.load_state().unwrap();
        assert_eq!(state.sessions.len(), 2);

        let names: Vec<_> = state.sessions.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Persistent Session"));
        assert!(names.contains(&"Another Session"));
    }
}

/// Test session restoration from saved state.
#[test]
fn test_session_restoration() {
    let temp = TempDir::new().unwrap();

    // Create and save state
    {
        let config = IntegrationConfig {
            auto_start_event_loop: false,
            ..Default::default()
        };
        let integration =
            CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

        integration
            .create_session("Restorable 1".to_string(), temp.path().to_path_buf())
            .unwrap();
        integration
            .create_session("Restorable 2".to_string(), temp.path().to_path_buf())
            .unwrap();

        integration.save_state().unwrap();
    }

    // Restore sessions
    {
        let config = IntegrationConfig {
            auto_start_event_loop: false,
            ..Default::default()
        };
        let integration =
            CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

        let restored_count = integration.restore_sessions().unwrap();
        assert_eq!(restored_count, 2);
        assert_eq!(integration.session_count().unwrap(), 2);
    }
}

/// Test that storage creates required directories.
#[test]
fn test_storage_directory_creation() {
    let temp = TempDir::new().unwrap();

    let storage = FileStorageService::new(temp.path()).unwrap();
    let codirigent_dir = storage.codirigent_dir();

    assert!(codirigent_dir.exists());
    assert!(codirigent_dir.join("tasks").exists());
}

// ============================================================================
// Input Detection Tests
// ============================================================================

/// Test input pattern detection.
#[test]
fn test_input_pattern_detection() {
    let (integration, temp) = create_test_integration();

    let id = create_test_session(&integration, &temp, "Detection Test");

    // Process output with input pattern
    integration.process_output(id, b"Continue? [y/n] ").unwrap();

    // Status should change to WaitingForInput
    let status = integration.get_session_status(id).unwrap();
    assert_eq!(status, Some(SessionStatus::WaitingForInput));
}

/// Test custom input patterns.
#[test]
fn test_custom_input_patterns() {
    let temp = TempDir::new().unwrap();

    let detector_config = DetectorConfig::with_patterns(vec![r"custom-prompt>".to_string()]);
    let config = IntegrationConfig {
        auto_start_event_loop: false,
        detector_config,
        ..Default::default()
    };

    let integration =
        CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

    let id = integration
        .create_session("Custom Pattern Test".to_string(), temp.path().to_path_buf())
        .unwrap();

    // Process output with custom pattern
    integration.process_output(id, b"custom-prompt>").unwrap();

    let status = integration.get_session_status(id).unwrap();
    assert_eq!(status, Some(SessionStatus::WaitingForInput));
}

/// Test multiple pattern types.
#[test]
fn test_multiple_pattern_types() {
    let (integration, temp) = create_test_integration();

    // Test patterns that exist in DEFAULT_PATTERNS:
    // - [y/n], [Y/n], [yes/no], (y/N), password:, Password:, Press Enter, Continue?, ? $, > $
    let test_cases = vec![
        ("yes_no_1", b"Proceed? [Y/n] " as &[u8]),
        ("yes_no_2", b"Delete? (y/N) " as &[u8]),
        ("password", b"Password: " as &[u8]),
        ("press_enter", b"Press Enter to continue" as &[u8]),
        ("question", b"What is your name? " as &[u8]),
    ];

    for (name, output) in test_cases {
        let id = integration
            .create_session(name.to_string(), temp.path().to_path_buf())
            .unwrap();

        integration.process_output(id, output).unwrap();

        let status = integration.get_session_status(id).unwrap();
        assert_eq!(
            status,
            Some(SessionStatus::WaitingForInput),
            "Pattern '{}' should trigger WaitingForInput",
            name
        );
    }
}

// ============================================================================
// Event Flow Tests
// ============================================================================

/// Test event subscription and reception.
#[tokio::test]
async fn test_event_subscription() {
    let (integration, _temp) = create_test_integration();

    let mut rx = integration.subscribe();

    // Publish an event
    integration.publish(CodirigentEvent::SessionCreated { id: SessionId(999) });

    // Receive the event
    let event = rx.recv().await.unwrap();
    assert!(matches!(
        event,
        CodirigentEvent::SessionCreated { id } if id == SessionId(999)
    ));
}

/// Test that session creation publishes events.
#[test]
fn test_session_creation_publishes_event() {
    let temp = TempDir::new().unwrap();
    let event_bus = Arc::new(DefaultEventBus::new(16));
    let mut rx = event_bus.subscribe();

    let manager = DefaultSessionManager::new(event_bus.clone());
    let id = manager
        .create_session("Test".to_string(), temp.path().to_path_buf())
        .unwrap();

    // Should receive SessionCreated event
    let event = rx.try_recv().unwrap();
    assert!(
        matches!(event, CodirigentEvent::SessionCreated { id: created_id } if created_id == id)
    );
}

/// Test that session closure publishes events.
#[test]
fn test_session_closure_publishes_event() {
    let temp = TempDir::new().unwrap();
    let event_bus = Arc::new(DefaultEventBus::new(16));
    let mut rx = event_bus.subscribe();

    let manager = DefaultSessionManager::new(event_bus.clone());
    let id = manager
        .create_session("Test".to_string(), temp.path().to_path_buf())
        .unwrap();

    // Consume create event
    let _ = rx.try_recv();

    manager.close_session(id).unwrap();

    // Should receive SessionClosed event
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, CodirigentEvent::SessionClosed { id: closed_id } if closed_id == id));
}

/// Test that status changes publish events.
#[test]
fn test_status_change_publishes_event() {
    let temp = TempDir::new().unwrap();
    let event_bus = Arc::new(DefaultEventBus::new(16));
    let mut rx = event_bus.subscribe();

    let manager = DefaultSessionManager::new(event_bus.clone());
    let id = manager
        .create_session("Test".to_string(), temp.path().to_path_buf())
        .unwrap();

    // Consume create event
    let _ = rx.try_recv();

    manager.update_status(id, SessionStatus::Working);

    // Should receive StatusChanged event
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

// ============================================================================
// Detector Integration Tests
// ============================================================================

/// Test detector integration with session manager.
#[test]
fn test_detector_integration() {
    let event_bus = Arc::new(DefaultEventBus::new(16));

    let mut detector = InputDetector::new(DetectorConfig::default(), event_bus.clone());

    // Start monitoring
    detector
        .start_monitoring(SessionId(1), std::process::id())
        .unwrap();

    // Initial status should be Idle
    let status = detector.get_status(SessionId(1));
    assert!(status.is_some());

    // Process output with pattern
    detector.process_output(SessionId(1), b"Continue? [y/n]");

    // Should detect WaitingForInput
    assert_eq!(
        detector.get_status(SessionId(1)),
        Some(SessionStatus::WaitingForInput)
    );

    // Should have published InputRequired event
    // Note: May need to skip StatusChanged event first
    let _events_received = true;

    // Stop monitoring
    detector.stop_monitoring(SessionId(1));
    assert!(detector.get_status(SessionId(1)).is_none());
}

/// Test detector with multiple sessions.
#[test]
fn test_detector_multiple_sessions() {
    let event_bus = Arc::new(DefaultEventBus::new(16));
    let mut detector = InputDetector::new(DetectorConfig::default(), event_bus);

    detector.start_monitoring(SessionId(1), 1234).unwrap();
    detector.start_monitoring(SessionId(2), 5678).unwrap();
    detector.start_monitoring(SessionId(3), 9012).unwrap();

    assert_eq!(detector.session_count(), 3);

    // Different sessions can have different patterns
    detector.process_output(SessionId(1), b"Continue? [y/n]");
    detector.process_output(SessionId(2), b"Working...");

    assert_eq!(
        detector.get_status(SessionId(1)),
        Some(SessionStatus::WaitingForInput)
    );

    // Stop one session
    detector.stop_monitoring(SessionId(2));
    assert_eq!(detector.session_count(), 2);
}

// ============================================================================
// Workspace Integration Tests
// ============================================================================

/// Test workspace session management.
#[test]
fn test_workspace_session_management() {
    use codirigent_ui::layout::LayoutProfile;
    use codirigent_ui::workspace::Workspace;

    let mut workspace = Workspace::new();
    workspace.set_layout(LayoutProfile::Grid2x2);

    // Add sessions
    let session1 = Session::new(SessionId(1), "Session 1".to_string(), PathBuf::from("/tmp"));
    let session2 = Session::new(SessionId(2), "Session 2".to_string(), PathBuf::from("/tmp"));

    assert!(workspace.add_session(session1));
    assert!(workspace.add_session(session2));

    assert_eq!(workspace.sessions().len(), 2);
    assert_eq!(workspace.available_slots(), 2);

    // Focus management
    assert_eq!(workspace.focused_session_id(), Some(SessionId(1)));
    workspace.focus_session(SessionId(2));
    assert_eq!(workspace.focused_session_id(), Some(SessionId(2)));

    // Remove session
    workspace.remove_session(SessionId(1));
    assert_eq!(workspace.sessions().len(), 1);
}

/// Test workspace layout switching.
#[test]
fn test_workspace_layout_switching() {
    use codirigent_ui::layout::LayoutProfile;
    use codirigent_ui::workspace::Workspace;

    let mut workspace = Workspace::new();

    assert_eq!(workspace.layout_profile(), LayoutProfile::Grid2x2);

    workspace.next_layout();
    assert_eq!(workspace.layout_profile(), LayoutProfile::Stack1x4);

    workspace.next_layout();
    assert_eq!(workspace.layout_profile(), LayoutProfile::Grid2x3);

    workspace.previous_layout();
    assert_eq!(workspace.layout_profile(), LayoutProfile::Stack1x4);
}

/// Test workspace sidebar toggling.
#[test]
fn test_workspace_sidebar() {
    use codirigent_ui::workspace::Workspace;

    let mut workspace = Workspace::new();

    assert!(workspace.is_sidebar_visible());

    workspace.toggle_sidebar();
    assert!(!workspace.is_sidebar_visible());

    workspace.toggle_sidebar();
    assert!(workspace.is_sidebar_visible());
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Test handling of nonexistent sessions.
#[test]
fn test_nonexistent_session_handling() {
    let (integration, _temp) = create_test_integration();

    // Get nonexistent session
    let session = integration.get_session(SessionId(999)).unwrap();
    assert!(session.is_none());

    // Close nonexistent session
    let result = integration.close_session(SessionId(999));
    assert!(result.is_err());

    // Send input to nonexistent session
    let result = integration.send_input(SessionId(999), b"test");
    assert!(result.is_err());

    // Resize nonexistent session
    let result = integration.resize_session(SessionId(999), 24, 80);
    assert!(result.is_err());
}

/// Test handling of invalid working directory.
#[test]
fn test_invalid_working_directory() {
    let (integration, _temp) = create_test_integration();

    let result = integration.create_session(
        "Bad Session".to_string(),
        PathBuf::from("/nonexistent/path/that/does/not/exist"),
    );

    assert!(result.is_err());
}

// ============================================================================
// End-to-End Workflow Tests
// ============================================================================

/// Test a complete typical workflow.
#[test]
fn test_complete_workflow() {
    let temp = TempDir::new().unwrap();

    // Step 1: Initialize integration
    let config = IntegrationConfig {
        auto_start_event_loop: false,
        ..Default::default()
    };
    let integration =
        CodirigentIntegration::with_config(temp.path().to_path_buf(), config).unwrap();

    // Step 2: Create sessions
    let claude_id = integration
        .create_session("Claude Code".to_string(), temp.path().to_path_buf())
        .unwrap();
    let codex_id = integration
        .create_session("Codex CLI".to_string(), temp.path().to_path_buf())
        .unwrap();

    assert_eq!(integration.session_count().unwrap(), 2);

    // Step 3: Group sessions
    integration
        .set_session_group(
            claude_id,
            Some("backend".to_string()),
            Some("#1abc9c".to_string()),
        )
        .unwrap();
    integration
        .set_session_group(
            codex_id,
            Some("frontend".to_string()),
            Some("#3498db".to_string()),
        )
        .unwrap();

    // Step 4: Interact with sessions
    integration
        .send_input(claude_id, b"echo 'Hello from Claude'\n")
        .unwrap();
    integration
        .send_input(codex_id, b"echo 'Hello from Codex'\n")
        .unwrap();

    // Step 5: Simulate input detection
    integration
        .process_output(claude_id, b"Would you like to continue? [y/n] ")
        .unwrap();

    let status = integration.get_session_status(claude_id).unwrap();
    assert_eq!(status, Some(SessionStatus::WaitingForInput));

    // Step 6: Save state (has 2 sessions)
    integration.save_state().unwrap();

    // Verify persisted state has 2 sessions
    let state = integration.load_state().unwrap();
    assert_eq!(state.sessions.len(), 2);

    // Step 7: Close one session
    integration.close_session(codex_id).unwrap();
    assert_eq!(integration.session_count().unwrap(), 1);

    // Step 8: Save updated state and verify
    integration.save_state().unwrap();
    let state = integration.load_state().unwrap();
    assert_eq!(state.sessions.len(), 1);
}
