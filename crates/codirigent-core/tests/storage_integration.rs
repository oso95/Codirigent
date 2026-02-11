//! Integration tests for the FileStorageService.
//!
//! These tests verify the full storage workflow including:
//! - State persistence and recovery
//! - Task management lifecycle
//! - Concurrent storage instances
//! - Error handling scenarios

use codirigent_core::{
    AppState, FileStorageService, LayoutMode, RetryConfig, Session, SessionId, SessionStatus,
    StorageService, Task, TaskId, TaskPriority, TaskStatus,
};
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a test task.
fn create_task(id: &str, title: &str) -> Task {
    Task {
        id: TaskId(id.to_string()),
        title: title.to_string(),
        description: format!("Description for {}", title),
        priority: TaskPriority::Medium,
        status: TaskStatus::Queued,
        dependencies: vec![],
        tags: vec!["integration-test".to_string()],
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

/// Helper to create a test session.
fn create_session(id: u64, name: &str, dir: &str) -> Session {
    Session::new(SessionId(id), name.to_string(), PathBuf::from(dir))
}

#[test]
fn test_full_storage_workflow() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    // Initially empty
    let state = storage.load_state().unwrap();
    assert!(state.sessions.is_empty());

    // Add sessions and save
    let mut state = state;
    state
        .sessions
        .push(create_session(1, "Session 1", "/project1"));
    state
        .sessions
        .push(create_session(2, "Session 2", "/project2"));
    storage.save_state(&state).unwrap();

    // Reload and verify
    let loaded = storage.load_state().unwrap();
    assert_eq!(loaded.sessions.len(), 2);
    assert_eq!(loaded.sessions[0].name, "Session 1");
    assert_eq!(loaded.sessions[1].name, "Session 2");

    // Simulate crash recovery
    let storage2 = FileStorageService::new(temp.path()).unwrap();
    let recovered = storage2.load_state().unwrap();
    assert_eq!(recovered.sessions.len(), 2);
}

#[test]
fn test_task_lifecycle() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    // Create task
    let task = create_task("task-001", "Initial Task");
    storage.save_task(&task).unwrap();

    // Verify creation
    let loaded = storage.load_task(&TaskId("task-001".to_string())).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().title, "Initial Task");

    // Update task
    let mut updated_task = create_task("task-001", "Updated Task");
    updated_task.status = TaskStatus::Working;
    updated_task.assigned_session = Some(SessionId(1));
    storage.save_task(&updated_task).unwrap();

    // Verify update
    let loaded = storage
        .load_task(&TaskId("task-001".to_string()))
        .unwrap()
        .unwrap();
    assert_eq!(loaded.title, "Updated Task");
    assert_eq!(loaded.status, TaskStatus::Working);
    assert_eq!(loaded.assigned_session, Some(SessionId(1)));

    // List tasks
    let ids = storage.list_task_ids().unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].0, "task-001");

    // Delete task
    storage
        .delete_task(&TaskId("task-001".to_string()))
        .unwrap();
    assert!(storage
        .load_task(&TaskId("task-001".to_string()))
        .unwrap()
        .is_none());
    assert!(storage.list_task_ids().unwrap().is_empty());
}

#[test]
fn test_multiple_tasks() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    // Create multiple tasks
    for i in 1..=10 {
        let task = create_task(&format!("task-{:03}", i), &format!("Task {}", i));
        storage.save_task(&task).unwrap();
    }

    // List and verify order
    let ids = storage.list_task_ids().unwrap();
    assert_eq!(ids.len(), 10);
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(id.0, format!("task-{:03}", i + 1));
    }

    // Delete some tasks
    storage
        .delete_task(&TaskId("task-003".to_string()))
        .unwrap();
    storage
        .delete_task(&TaskId("task-007".to_string()))
        .unwrap();

    let ids = storage.list_task_ids().unwrap();
    assert_eq!(ids.len(), 8);
    assert!(!ids.iter().any(|id| id.0 == "task-003"));
    assert!(!ids.iter().any(|id| id.0 == "task-007"));
}

#[test]
fn test_state_with_layout_modes() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    // Test Grid layout
    let mut state = AppState {
        layout: LayoutMode::Grid { rows: 3, cols: 3 },
        ..Default::default()
    };
    storage.save_state(&state).unwrap();
    let loaded = storage.load_state().unwrap();
    assert!(matches!(
        loaded.layout,
        LayoutMode::Grid { rows: 3, cols: 3 }
    ));

    // Test Single layout
    state.layout = LayoutMode::Single;
    storage.save_state(&state).unwrap();
    let loaded = storage.load_state().unwrap();
    assert!(matches!(loaded.layout, LayoutMode::Single));

    // Test Custom layout
    state.layout = LayoutMode::Custom {
        positions: vec![
            (
                SessionId(1),
                codirigent_core::GridPosition { row: 0, col: 0 },
            ),
            (
                SessionId(2),
                codirigent_core::GridPosition { row: 0, col: 1 },
            ),
        ],
    };
    storage.save_state(&state).unwrap();
    let loaded = storage.load_state().unwrap();
    if let LayoutMode::Custom { positions } = loaded.layout {
        assert_eq!(positions.len(), 2);
    } else {
        panic!("Expected Custom layout");
    }
}

#[test]
fn test_session_with_all_status_types() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    let statuses = [
        SessionStatus::Idle,
        SessionStatus::Working,
        SessionStatus::NeedsAttention,
        SessionStatus::Error,
    ];

    for (i, status) in statuses.iter().enumerate() {
        let mut state = AppState::default();
        let mut session = create_session(i as u64, &format!("Session {}", i), "/tmp");
        session.status = *status;
        state.sessions.push(session);
        storage.save_state(&state).unwrap();

        let loaded = storage.load_state().unwrap();
        assert_eq!(loaded.sessions[0].status, *status);
    }
}

#[test]
fn test_concurrent_storage_instances() {
    let temp = TempDir::new().unwrap();

    // Create first storage and save state
    let storage1 = FileStorageService::new(temp.path()).unwrap();
    let mut state = AppState::default();
    state
        .sessions
        .push(create_session(1, "From Storage 1", "/project1"));
    storage1.save_state(&state).unwrap();

    // Create second storage and load
    let storage2 = FileStorageService::new(temp.path()).unwrap();
    let loaded = storage2.load_state().unwrap();
    assert_eq!(loaded.sessions.len(), 1);

    // Modify and save from storage2
    let mut state = loaded;
    state
        .sessions
        .push(create_session(2, "From Storage 2", "/project2"));
    storage2.save_state(&state).unwrap();

    // Load from storage1 and verify
    let loaded = storage1.load_state().unwrap();
    assert_eq!(loaded.sessions.len(), 2);
}

#[test]
fn test_from_codirigent_dir_constructor() {
    let temp = TempDir::new().unwrap();

    // First create using new() to set up directory
    let storage1 = FileStorageService::new(temp.path()).unwrap();
    let mut state = AppState::default();
    state.sessions.push(create_session(1, "Test", "/test"));
    storage1.save_state(&state).unwrap();

    // Then use from_codirigent_dir
    let dirigent_path = temp.path().join(".codirigent");
    let storage2 = FileStorageService::from_codirigent_dir(dirigent_path).unwrap();
    let loaded = storage2.load_state().unwrap();
    assert_eq!(loaded.sessions.len(), 1);
}

#[test]
fn test_task_with_dependencies() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    // Create parent tasks
    let parent1 = create_task("parent-001", "Parent Task 1");
    let parent2 = create_task("parent-002", "Parent Task 2");
    storage.save_task(&parent1).unwrap();
    storage.save_task(&parent2).unwrap();

    // Create child task with dependencies
    let mut child = create_task("child-001", "Child Task");
    child.dependencies = vec![
        TaskId("parent-001".to_string()),
        TaskId("parent-002".to_string()),
    ];
    storage.save_task(&child).unwrap();

    // Load and verify
    let loaded = storage
        .load_task(&TaskId("child-001".to_string()))
        .unwrap()
        .unwrap();
    assert_eq!(loaded.dependencies.len(), 2);
    assert!(loaded
        .dependencies
        .contains(&TaskId("parent-001".to_string())));
    assert!(loaded
        .dependencies
        .contains(&TaskId("parent-002".to_string())));
}

#[test]
fn test_state_persistence_after_updates() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    // Initial state
    let mut state = AppState::default();
    state
        .sessions
        .push(create_session(1, "Session 1", "/project1"));
    storage.save_state(&state).unwrap();

    // Multiple updates
    for i in 2..=5 {
        let mut state = storage.load_state().unwrap();
        state.sessions.push(create_session(
            i,
            &format!("Session {}", i),
            &format!("/project{}", i),
        ));
        storage.save_state(&state).unwrap();
    }

    // Verify final state
    let final_state = storage.load_state().unwrap();
    assert_eq!(final_state.sessions.len(), 5);
    for i in 1..=5 {
        assert!(final_state.sessions.iter().any(|s| s.id == SessionId(i)));
    }
}

#[test]
fn test_storage_directory_structure() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    // Save state and tasks
    storage.save_state(&AppState::default()).unwrap();
    storage
        .save_task(&create_task("task-001", "Task 1"))
        .unwrap();

    // Verify directory structure
    let codirigent_dir = temp.path().join(".codirigent");
    assert!(codirigent_dir.exists());
    assert!(codirigent_dir.join("state.json").exists());
    assert!(codirigent_dir.join("tasks").exists());
    assert!(codirigent_dir.join("tasks").join("task-001.json").exists());
}

#[test]
fn test_task_priority_persistence() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    let priorities = [
        TaskPriority::Critical,
        TaskPriority::High,
        TaskPriority::Medium,
        TaskPriority::Low,
    ];

    for (i, priority) in priorities.iter().enumerate() {
        let mut task = create_task(&format!("priority-{}", i), &format!("Priority {} Task", i));
        task.priority = *priority;
        storage.save_task(&task).unwrap();

        let loaded = storage.load_task(&task.id).unwrap().unwrap();
        assert_eq!(loaded.priority, *priority);
    }
}

#[test]
fn test_task_status_workflow() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    // Simulate task workflow
    let mut task = create_task("workflow-001", "Workflow Task");
    assert_eq!(task.status, TaskStatus::Queued);
    storage.save_task(&task).unwrap();

    // Assign to session
    task.status = TaskStatus::Assigned;
    task.assigned_session = Some(SessionId(1));
    task.assigned_at = Some(chrono::Utc::now());
    storage.save_task(&task).unwrap();

    // Start working
    task.status = TaskStatus::Working;
    task.started_at = Some(chrono::Utc::now());
    storage.save_task(&task).unwrap();

    // Verify
    task.status = TaskStatus::Verifying;
    storage.save_task(&task).unwrap();

    // Complete
    task.status = TaskStatus::Done;
    task.completed_at = Some(chrono::Utc::now());
    storage.save_task(&task).unwrap();

    // Load and verify final state
    let loaded = storage.load_task(&task.id).unwrap().unwrap();
    assert_eq!(loaded.status, TaskStatus::Done);
    assert!(loaded.assigned_session.is_some());
    assert!(loaded.assigned_at.is_some());
    assert!(loaded.started_at.is_some());
    assert!(loaded.completed_at.is_some());
}

#[test]
fn test_empty_state_and_tasks() {
    let temp = TempDir::new().unwrap();
    let storage = FileStorageService::new(temp.path()).unwrap();

    // Empty state should work
    let state = storage.load_state().unwrap();
    assert!(state.sessions.is_empty());

    // Saving empty state should work
    storage.save_state(&state).unwrap();

    // Empty task list should work
    let ids = storage.list_task_ids().unwrap();
    assert!(ids.is_empty());
}
