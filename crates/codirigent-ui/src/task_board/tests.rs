//! Tests for task board components.

use super::*;
use crate::sidebar::Color;

// === Panel Tests ===

#[test]
fn test_task_board_panel_new() {
    let panel = TaskBoardPanel::new();
    assert_eq!(panel.active_tab(), TaskBoardTab::Queue);
    assert!(!panel.is_auto_assign_enabled());
    assert!(!panel.is_expanded()); // Panel starts collapsed by default
}

#[test]
fn test_task_board_panel_default() {
    let panel = TaskBoardPanel::default();
    assert_eq!(panel.active_tab(), TaskBoardTab::Queue);
}

#[test]
fn test_task_board_tab_labels() {
    assert_eq!(TaskBoardTab::Queue.label(), "Queue");
    assert_eq!(TaskBoardTab::InProgress.label(), "In Progress");
    assert_eq!(TaskBoardTab::Review.label(), "Review");
    assert_eq!(TaskBoardTab::Done.label(), "Done");
}

#[test]
fn test_task_board_tab_icons() {
    assert_eq!(TaskBoardTab::Queue.icon(), "[Q]");
    assert_eq!(TaskBoardTab::InProgress.icon(), "[IP]");
    assert_eq!(TaskBoardTab::Review.icon(), "[RV]");
    assert_eq!(TaskBoardTab::Done.icon(), "[OK]");
}

#[test]
fn test_click_tab() {
    let mut panel = TaskBoardPanel::new();
    panel.click_tab(TaskBoardTab::InProgress);
    assert_eq!(panel.active_tab(), TaskBoardTab::InProgress);

    let events = panel.take_events();
    assert!(matches!(
        &events[0],
        TaskBoardEvent::TabSelected(TaskBoardTab::InProgress)
    ));
}

#[test]
fn test_toggle_auto_assign() {
    let mut panel = TaskBoardPanel::new();
    assert!(!panel.is_auto_assign_enabled());

    panel.toggle_auto_assign();
    assert!(panel.is_auto_assign_enabled());

    let events = panel.take_events();
    assert!(matches!(
        &events[0],
        TaskBoardEvent::AutoAssignToggled(true)
    ));
}

#[test]
fn test_set_auto_assign() {
    let mut panel = TaskBoardPanel::new();
    panel.set_auto_assign(true);
    assert!(panel.is_auto_assign_enabled());

    // Setting to same value should not emit event
    panel.take_events();
    panel.set_auto_assign(true);
    assert!(panel.take_events().is_empty());
}

#[test]
fn test_click_add_task() {
    let mut panel = TaskBoardPanel::new();
    panel.click_add_task();

    let events = panel.take_events();
    assert!(matches!(&events[0], TaskBoardEvent::AddTaskClicked));
}

#[test]
fn test_toggle_expanded() {
    let mut panel = TaskBoardPanel::new();
    assert!(!panel.is_expanded()); // Starts collapsed
    assert_eq!(panel.height(), TaskBoardPanel::DEFAULT_COLLAPSED_HEIGHT);

    panel.toggle_expanded();
    assert!(panel.is_expanded());
    assert_eq!(panel.height(), TaskBoardPanel::DEFAULT_EXPANDED_HEIGHT);
}

#[test]
fn test_set_task_counts() {
    let mut panel = TaskBoardPanel::new();
    panel.set_task_counts(5, 2, 1, 10);

    assert_eq!(panel.task_count(TaskBoardTab::Queue), 5);
    assert_eq!(panel.task_count(TaskBoardTab::InProgress), 2);
    assert_eq!(panel.task_count(TaskBoardTab::Review), 1);
    assert_eq!(panel.task_count(TaskBoardTab::Done), 10);

    // Tabs should reflect counts
    let tabs = panel.tabs();
    assert_eq!(tabs[0].count, 5);
    assert_eq!(tabs[1].count, 2);
}

#[test]
fn test_select_task() {
    let mut panel = TaskBoardPanel::new();
    panel.select_task("task-123");

    let events = panel.take_events();
    assert!(matches!(
        &events[0],
        TaskBoardEvent::TaskSelected(id) if id == "task-123"
    ));
}

#[test]
fn test_trigger_task_action() {
    let mut panel = TaskBoardPanel::new();
    panel.trigger_task_action("task-123", TaskAction::Assign);

    let events = panel.take_events();
    assert!(matches!(
        &events[0],
        TaskBoardEvent::TaskAction { task_id, action }
        if task_id == "task-123" && *action == TaskAction::Assign
    ));
}

#[test]
fn test_render_hints() {
    let mut panel = TaskBoardPanel::new();
    panel.set_task_counts(3, 1, 0, 5);
    panel.toggle_auto_assign();

    let hints = panel.render_hints();
    assert_eq!(hints.active_tab, TaskBoardTab::Queue);
    assert!(hints.auto_assign.enabled);
    assert!(!hints.is_expanded); // Panel starts collapsed by default
    assert_eq!(hints.tabs.len(), 4);
}

#[test]
fn test_tab_button_new() {
    let tab = TabButton::new(TaskBoardTab::Queue, true, 5);
    assert_eq!(tab.label, "Queue");
    assert!(tab.is_active);
    assert!(!tab.is_hovered);
    assert_eq!(tab.count, 5);
}

#[test]
fn test_auto_assign_toggle_colors() {
    let enabled = AutoAssignToggle::new(true);
    let disabled = AutoAssignToggle::new(false);

    // Different colors for enabled/disabled
    assert!(enabled.background_color().g > 0.7); // Teal has high green
    assert!(disabled.background_color().g < 0.2); // Border color is dark
}

#[test]
fn test_set_expanded_height() {
    let mut panel = TaskBoardPanel::new();
    // Panel starts collapsed, expand it first
    panel.toggle_expanded();
    panel.set_expanded_height(300.0);
    assert_eq!(panel.height(), 300.0);

    // Minimum height enforced
    panel.set_expanded_height(20.0);
    assert!(panel.height() > TaskBoardPanel::HEADER_HEIGHT);
}

// === Task Item Tests ===

#[test]
fn test_task_item_new() {
    let item = TaskItem::new("task-1", "Fix bug in login");
    assert_eq!(item.id, "task-1");
    assert_eq!(item.title, "Fix bug in login");
    assert_eq!(item.priority, TaskPriority::Medium);
    assert_eq!(item.status, TaskStatus::Queued);
}

#[test]
fn test_task_item_builder() {
    let item = TaskItem::new("task-1", "Feature")
        .with_priority(TaskPriority::High)
        .with_status(TaskStatus::InProgress)
        .with_simple_tag("frontend")
        .with_assigned_to("Session 1")
        .with_estimated_time("2h");

    assert_eq!(item.priority, TaskPriority::High);
    assert_eq!(item.status, TaskStatus::InProgress);
    assert_eq!(item.tags.len(), 1);
    assert_eq!(item.assigned_to, Some("Session 1".to_string()));
    assert_eq!(item.estimated_time, Some("2h".to_string()));
}

#[test]
fn test_task_priority_colors() {
    let high = TaskPriority::High.color();
    let medium = TaskPriority::Medium.color();
    let low = TaskPriority::Low.color();

    // All should have distinct colors
    assert!(high.r > 0.9); // Red
    assert!(medium.r > 0.8 && medium.g > 0.5); // Orange
    assert!(low.b > 0.8); // Blue
}

#[test]
fn test_task_status_labels() {
    assert_eq!(TaskStatus::Queued.label(), "Queued");
    assert_eq!(TaskStatus::InProgress.label(), "In Progress");
    assert_eq!(TaskStatus::PendingReview.label(), "Pending Review");
    assert_eq!(TaskStatus::Completed.label(), "Completed");
}

#[test]
fn test_task_tag() {
    let tag = TaskTag::new("urgent", Color::from_hex("#FF0000"));
    assert_eq!(tag.text, "urgent");

    let simple = TaskTag::simple("feature");
    assert_eq!(simple.text, "feature");
}

#[test]
fn test_available_actions_queued() {
    let item = TaskItem::new("t1", "Task").with_status(TaskStatus::Queued);
    let actions = item.available_actions();
    assert!(actions.contains(&TaskItemAction::Assign));
    assert!(actions.contains(&TaskItemAction::Edit));
    assert!(actions.contains(&TaskItemAction::Delete));
}

#[test]
fn test_available_actions_in_progress() {
    let item = TaskItem::new("t1", "Task").with_status(TaskStatus::InProgress);
    let actions = item.available_actions();
    assert!(actions.contains(&TaskItemAction::MarkForReview));
    assert!(actions.contains(&TaskItemAction::Edit));
}

#[test]
fn test_available_actions_pending_review() {
    let item = TaskItem::new("t1", "Task").with_status(TaskStatus::PendingReview);
    let actions = item.available_actions();
    assert!(actions.contains(&TaskItemAction::Approve));
    assert!(actions.contains(&TaskItemAction::Reject));
}

#[test]
fn test_available_actions_completed() {
    let item = TaskItem::new("t1", "Task").with_status(TaskStatus::Completed);
    let actions = item.available_actions();
    assert!(actions.contains(&TaskItemAction::Reopen));
    assert!(actions.contains(&TaskItemAction::Delete));
}

#[test]
fn test_task_item_render_hints() {
    let item = TaskItem::new("t1", "Task")
        .with_priority(TaskPriority::High)
        .with_simple_tag("bug");

    let hints = item.render_hints();
    assert_eq!(hints.id, "t1");
    assert_eq!(hints.title, "Task");
    assert_eq!(hints.priority.priority, TaskPriority::High);
    assert_eq!(hints.tags.len(), 1);
}

#[test]
fn test_task_item_action_labels() {
    assert_eq!(TaskItemAction::Assign.label(), "Assign");
    assert_eq!(TaskItemAction::Edit.label(), "Edit");
    assert_eq!(TaskItemAction::Delete.label(), "Delete");
}

#[test]
fn test_priority_indicator() {
    let item = TaskItem::new("t1", "Task").with_priority(TaskPriority::Low);
    let indicator = item.priority_indicator();
    assert_eq!(indicator.priority, TaskPriority::Low);
}

#[test]
fn test_status_badge() {
    let item = TaskItem::new("t1", "Task").with_status(TaskStatus::InProgress);
    let badge = item.status_badge();
    assert_eq!(badge.status, TaskStatus::InProgress);
    assert_eq!(badge.label, "In Progress");
}

#[test]
fn test_task_item_with_created_at() {
    let item = TaskItem::new("t1", "Task").with_created_at("2024-01-15T10:30:00Z");
    assert_eq!(item.created_at, Some("2024-01-15T10:30:00Z".to_string()));
}

#[test]
fn test_task_item_with_tag() {
    let tag = TaskTag::new("custom", Color::from_hex("#FF00FF"));
    let item = TaskItem::new("t1", "Task").with_tag(tag);
    assert_eq!(item.tags.len(), 1);
    assert_eq!(item.tags[0].text, "custom");
}
