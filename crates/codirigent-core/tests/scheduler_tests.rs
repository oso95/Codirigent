//! Task queue and scheduler tests.
//!
//! Tests verify priority ordering, dependency resolution, and different scheduling modes.

use codirigent_core::{
    DefaultEventBus, SchedulerConfig, SchedulerMode, Task, TaskId, TaskPriority, TaskQueue,
};
use std::sync::Arc;

/// Test priority-based task ordering.
///
/// Verifies that tasks with higher priority are selected first when using
/// Priority scheduling mode.
#[test]
fn test_priority_ordering() {
    let event_bus = Arc::new(DefaultEventBus::new(16));
    let config = SchedulerConfig {
        mode: SchedulerMode::Priority,
        ..Default::default()
    };
    let mut queue = TaskQueue::new(config, event_bus);

    // Create tasks with different priorities
    let mut low = Task::new(
        TaskId("low".to_string()),
        "Low priority task".to_string(),
        "This is low priority".to_string(),
    );
    low.priority = TaskPriority::Low;

    let mut high = Task::new(
        TaskId("high".to_string()),
        "High priority task".to_string(),
        "This is high priority".to_string(),
    );
    high.priority = TaskPriority::High;

    let mut medium = Task::new(
        TaskId("medium".to_string()),
        "Medium priority task".to_string(),
        "This is medium priority".to_string(),
    );
    medium.priority = TaskPriority::Medium;

    let mut critical = Task::new(
        TaskId("critical".to_string()),
        "Critical priority task".to_string(),
        "This is critical priority".to_string(),
    );
    critical.priority = TaskPriority::Critical;

    // Enqueue in random order
    queue.enqueue(low.clone()).unwrap();
    queue.enqueue(medium.clone()).unwrap();
    queue.enqueue(critical.clone()).unwrap();
    queue.enqueue(high.clone()).unwrap();

    // Should get critical first
    let next = queue.next_task(&[]).unwrap();
    assert_eq!(next.id, TaskId("critical".to_string()));

    // Mark as completed
    queue.complete_task(&TaskId("critical".to_string()), true).unwrap();

    // Should get high next
    let next = queue.next_task(&[]).unwrap();
    assert_eq!(next.id, TaskId("high".to_string()));

    queue.complete_task(&TaskId("high".to_string()), true).unwrap();

    // Should get medium next
    let next = queue.next_task(&[]).unwrap();
    assert_eq!(next.id, TaskId("medium".to_string()));

    queue.complete_task(&TaskId("medium".to_string()), true).unwrap();

    // Should get low last
    let next = queue.next_task(&[]).unwrap();
    assert_eq!(next.id, TaskId("low".to_string()));
}

/// Test FIFO (first-in, first-out) ordering.
///
/// Verifies that tasks are processed in the order they were added when using
/// FIFO scheduling mode.
#[test]
fn test_fifo_ordering() {
    let event_bus = Arc::new(DefaultEventBus::new(16));
    let config = SchedulerConfig {
        mode: SchedulerMode::Fifo,
        ..Default::default()
    };
    let mut queue = TaskQueue::new(config, event_bus);

    // Add tasks in specific order
    let task1 = Task::new(
        TaskId("1".to_string()),
        "First task".to_string(),
        "Added first".to_string(),
    );
    let task2 = Task::new(
        TaskId("2".to_string()),
        "Second task".to_string(),
        "Added second".to_string(),
    );
    let task3 = Task::new(
        TaskId("3".to_string()),
        "Third task".to_string(),
        "Added third".to_string(),
    );

    queue.enqueue(task1).unwrap();
    queue.enqueue(task2).unwrap();
    queue.enqueue(task3).unwrap();

    // Should get in FIFO order
    let next = queue.next_task(&[]).unwrap();
    assert_eq!(next.id, TaskId("1".to_string()));

    queue.complete_task(&TaskId("1".to_string()), true).unwrap();

    let next = queue.next_task(&[]).unwrap();
    assert_eq!(next.id, TaskId("2".to_string()));

    queue.complete_task(&TaskId("2".to_string()), true).unwrap();

    let next = queue.next_task(&[]).unwrap();
    assert_eq!(next.id, TaskId("3".to_string()));
}

/// Test dependency resolution.
///
/// Verifies that tasks with dependencies are blocked until their
/// dependencies are completed.
#[test]
fn test_dependency_blocking() {
    let event_bus = Arc::new(DefaultEventBus::new(16));
    let config = SchedulerConfig::default();
    let mut queue = TaskQueue::new(config, event_bus);

    let task1 = Task::new(
        TaskId("task1".to_string()),
        "Foundation task".to_string(),
        "Must complete first".to_string(),
    );

    let mut task2 = Task::new(
        TaskId("task2".to_string()),
        "Dependent task".to_string(),
        "Depends on task1".to_string(),
    );
    task2.dependencies = vec![TaskId("task1".to_string())];

    let mut task3 = Task::new(
        TaskId("task3".to_string()),
        "Double dependent".to_string(),
        "Depends on task2".to_string(),
    );
    task3.dependencies = vec![TaskId("task2".to_string())];

    // Enqueue in reverse order to test dependency resolution
    queue.enqueue(task3).unwrap();
    queue.enqueue(task2).unwrap();
    queue.enqueue(task1).unwrap();

    // Should get task1 first (no dependencies)
    let mut completed: Vec<TaskId> = Vec::new();
    let next = queue.next_task(&completed).unwrap();
    assert_eq!(next.id, TaskId("task1".to_string()));

    // Mark task1 as completed
    queue.complete_task(&TaskId("task1".to_string()), true).unwrap();
    completed.push(TaskId("task1".to_string()));

    // Now task2 should be available (dependency met)
    let next = queue.next_task(&completed).unwrap();
    assert_eq!(next.id, TaskId("task2".to_string()));

    queue.complete_task(&TaskId("task2".to_string()), true).unwrap();
    completed.push(TaskId("task2".to_string()));

    // Finally task3 should be available (dependency met)
    let next = queue.next_task(&completed).unwrap();
    assert_eq!(next.id, TaskId("task3".to_string()));
}

/// Test that completed tasks are no longer returned.
///
/// Verifies that once a task is marked as completed, it is not returned
/// by next_task() anymore.
#[test]
fn test_completed_tasks_not_returned() {
    let event_bus = Arc::new(DefaultEventBus::new(16));
    let config = SchedulerConfig::default();
    let mut queue = TaskQueue::new(config, event_bus);

    let task = Task::new(
        TaskId("task1".to_string()),
        "Single task".to_string(),
        "Will be completed".to_string(),
    );

    queue.enqueue(task).unwrap();

    // Get and complete the task
    let next = queue.next_task(&[]).unwrap();
    assert_eq!(next.id, TaskId("task1".to_string()));

    queue.complete_task(&TaskId("task1".to_string()), true).unwrap();

    // Should have no more tasks
    let next = queue.next_task(&[]);
    assert!(next.is_none(), "Completed task should not be returned");
}

/// Test empty queue behavior.
///
/// Verifies that next_task() returns None when the queue is empty.
#[test]
fn test_empty_queue() {
    let event_bus = Arc::new(DefaultEventBus::new(16));
    let config = SchedulerConfig::default();
    let queue = TaskQueue::new(config, event_bus);

    // Empty queue should return None
    let next = queue.next_task(&[]);
    assert!(next.is_none(), "Empty queue should return None");
}

/// Test multiple dependencies.
///
/// Verifies that a task with multiple dependencies is only available
/// when ALL dependencies are completed.
#[test]
fn test_multiple_dependencies() {
    let event_bus = Arc::new(DefaultEventBus::new(16));
    let config = SchedulerConfig::default();
    let mut queue = TaskQueue::new(config, event_bus);

    let task_a = Task::new(
        TaskId("A".to_string()),
        "Task A".to_string(),
        "First dependency".to_string(),
    );

    let task_b = Task::new(
        TaskId("B".to_string()),
        "Task B".to_string(),
        "Second dependency".to_string(),
    );

    let mut task_c = Task::new(
        TaskId("C".to_string()),
        "Task C".to_string(),
        "Depends on both A and B".to_string(),
    );
    task_c.dependencies = vec![
        TaskId("A".to_string()),
        TaskId("B".to_string()),
    ];

    queue.enqueue(task_a).unwrap();
    queue.enqueue(task_b).unwrap();
    queue.enqueue(task_c).unwrap();

    // Track completed tasks
    let mut completed: Vec<TaskId> = Vec::new();

    // Complete task A
    let next = queue.next_task(&completed).unwrap();
    assert!(next.id == TaskId("A".to_string()) || next.id == TaskId("B".to_string()));
    let task_id = next.id.clone();
    queue.complete_task(&task_id, true).unwrap();
    completed.push(task_id);

    // Task C should still be blocked (only 1 of 2 dependencies met)
    let next = queue.next_task(&completed).unwrap();
    assert!(next.id == TaskId("A".to_string()) || next.id == TaskId("B".to_string()));
    assert_ne!(next.id, TaskId("C".to_string()), "Task C should still be blocked");
    let task_id = next.id.clone();
    queue.complete_task(&task_id, true).unwrap();
    completed.push(task_id);

    // Now task C should be available (both dependencies met)
    let next = queue.next_task(&completed).unwrap();
    assert_eq!(next.id, TaskId("C".to_string()));
}

/// Test scheduler mode default.
///
/// Verifies that the default scheduling mode is Smart.
#[test]
fn test_scheduler_mode_default() {
    let mode = SchedulerMode::default();
    assert_eq!(mode, SchedulerMode::Smart);
}

/// Test scheduler config default values.
///
/// Verifies that SchedulerConfig has sensible defaults.
#[test]
fn test_scheduler_config_defaults() {
    let config = SchedulerConfig::default();

    assert_eq!(config.mode, SchedulerMode::Smart);
    assert!(config.auto_assign);
    assert!(!config.confirm_before_assign);
    assert_eq!(config.idle_threshold_seconds, 5);
    assert!((config.priority_weight - 0.5).abs() < 0.01);
    assert!((config.age_weight - 0.3).abs() < 0.01);
    assert!((config.tag_match_weight - 0.2).abs() < 0.01);
}

/// Test task priority enum values.
///
/// Verifies all task priority levels are available.
#[test]
fn test_task_priority_values() {
    let _low = TaskPriority::Low;
    let _medium = TaskPriority::Medium;
    let _high = TaskPriority::High;
    let _critical = TaskPriority::Critical;

    // Verify default is Medium
    let default_priority = TaskPriority::default();
    assert_eq!(default_priority, TaskPriority::Medium);
}
