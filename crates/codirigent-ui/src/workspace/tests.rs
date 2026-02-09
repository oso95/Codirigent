//! Tests for the workspace core module.

use super::core::*;
use crate::layout::{Bounds, FocusDirection, LayoutProfile, Point};
use crate::theme::CodirigentTheme;
use codirigent_core::{LayoutNode, Session, SessionId, SessionStatus, SlotId, SplitDirection};
use std::path::PathBuf;

fn make_session(id: u64, name: &str) -> Session {
    Session::new(SessionId(id), name.to_string(), PathBuf::from("/tmp"))
}

#[test]
fn test_workspace_new() {
    let ws = Workspace::new();
    assert_eq!(ws.layout_profile(), LayoutProfile::Grid2x2);
    assert!(ws.sessions().is_empty());
    assert!(ws.is_sidebar_visible());
}

#[test]
fn test_workspace_with_profile() {
    let ws = Workspace::with_profile(LayoutProfile::Grid3x3);
    assert_eq!(ws.layout_profile(), LayoutProfile::Grid3x3);
}

#[test]
fn test_workspace_layout_cycle() {
    let mut ws = Workspace::new();
    ws.next_layout();
    assert_eq!(ws.layout_profile(), LayoutProfile::Stack1x4);
    ws.previous_layout();
    assert_eq!(ws.layout_profile(), LayoutProfile::Grid2x2);
}

#[test]
fn test_workspace_set_layout() {
    let mut ws = Workspace::new();
    ws.set_layout(LayoutProfile::Single);
    assert_eq!(ws.layout_profile(), LayoutProfile::Single);
}

#[test]
fn test_workspace_add_session() {
    let mut ws = Workspace::new();
    let session = make_session(1, "Session 1");

    assert!(ws.add_session(session));
    assert_eq!(ws.sessions().len(), 1);
    assert_eq!(ws.focused_session_id(), Some(SessionId(1)));
}

#[test]
fn test_workspace_add_duplicate_session() {
    let mut ws = Workspace::new();
    ws.add_session(make_session(1, "Session 1"));

    // Adding duplicate should fail
    assert!(!ws.add_session(make_session(1, "Session 1 again")));
    assert_eq!(ws.sessions().len(), 1);
}

#[test]
fn test_workspace_add_session_full() {
    let mut ws = Workspace::with_profile(LayoutProfile::Grid2x2);

    // Fill all 4 slots
    for i in 1..=4 {
        assert!(ws.add_session(make_session(i, &format!("Session {}", i))));
    }

    // 5th should fail
    assert!(!ws.add_session(make_session(5, "Session 5")));
    assert_eq!(ws.sessions().len(), 4);
}

#[test]
fn test_workspace_remove_session() {
    let mut ws = Workspace::new();
    ws.add_session(make_session(1, "Session 1"));
    ws.add_session(make_session(2, "Session 2"));

    let removed = ws.remove_session(SessionId(1));
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, SessionId(1));
    assert_eq!(ws.sessions().len(), 1);

    // Remove non-existent
    assert!(ws.remove_session(SessionId(99)).is_none());
}

#[test]
fn test_workspace_session_access() {
    let mut ws = Workspace::new();
    ws.add_session(make_session(1, "Session 1"));

    assert!(ws.session(SessionId(1)).is_some());
    assert!(ws.session(SessionId(99)).is_none());

    // Mutable access
    assert!(ws.session_mut(SessionId(1)).is_some());
}

#[test]
fn test_workspace_update_status() {
    let mut ws = Workspace::new();
    ws.add_session(make_session(1, "Session 1"));

    ws.update_session_status(SessionId(1), SessionStatus::Working);
    assert_eq!(
        ws.session(SessionId(1)).unwrap().status,
        SessionStatus::Working
    );
}

#[test]
fn test_workspace_visible_sessions() {
    let mut ws = Workspace::new();
    ws.add_session(make_session(1, "Session 1"));
    ws.add_session(make_session(2, "Session 2"));

    let visible = ws.visible_sessions();
    assert_eq!(visible.len(), 2);
}

#[test]
fn test_workspace_available_slots() {
    let mut ws = Workspace::with_profile(LayoutProfile::Grid2x2);
    assert_eq!(ws.available_slots(), 4);

    ws.add_session(make_session(1, "Session 1"));
    assert_eq!(ws.available_slots(), 3);
}

#[test]
fn test_workspace_focus_session() {
    let mut ws = Workspace::new();
    ws.add_session(make_session(1, "Session 1"));
    ws.add_session(make_session(2, "Session 2"));

    assert!(ws.focus_session(SessionId(2)));
    assert_eq!(ws.focused_session_id(), Some(SessionId(2)));

    assert!(!ws.focus_session(SessionId(99)));
}

#[test]
fn test_workspace_focus_session_number() {
    let mut ws = Workspace::new();
    ws.add_session(make_session(1, "Session 1"));
    ws.add_session(make_session(2, "Session 2"));

    assert!(ws.focus_session_number(2));
    assert_eq!(ws.focused_session_id(), Some(SessionId(2)));

    // Invalid numbers
    assert!(!ws.focus_session_number(0));
    assert!(!ws.focus_session_number(10));
}

#[test]
fn test_workspace_focus_navigation() {
    let mut ws = Workspace::new();
    ws.add_session(make_session(1, "Session 1"));
    ws.add_session(make_session(2, "Session 2"));
    ws.add_session(make_session(3, "Session 3"));

    assert_eq!(ws.focused_session_id(), Some(SessionId(1)));

    ws.focus_next();
    assert_eq!(ws.focused_session_id(), Some(SessionId(2)));

    ws.focus_previous();
    assert_eq!(ws.focused_session_id(), Some(SessionId(1)));
}

#[test]
fn test_workspace_focus_direction() {
    let mut ws = Workspace::with_profile(LayoutProfile::Grid2x2);
    ws.add_session(make_session(1, "S1"));
    ws.add_session(make_session(2, "S2"));
    ws.add_session(make_session(3, "S3"));
    ws.add_session(make_session(4, "S4"));

    // Start at top-left (index 0)
    ws.focus_direction(FocusDirection::Right);
    assert_eq!(ws.focused_session_id(), Some(SessionId(2)));

    ws.focus_direction(FocusDirection::Down);
    assert_eq!(ws.focused_session_id(), Some(SessionId(4)));
}

#[test]
fn test_workspace_focused_session() {
    let mut ws = Workspace::new();
    ws.add_session(make_session(1, "Session 1"));

    let focused = ws.focused_session();
    assert!(focused.is_some());
    assert_eq!(focused.unwrap().id, SessionId(1));
}

#[test]
fn test_workspace_sidebar() {
    let mut ws = Workspace::new();

    assert!(ws.is_sidebar_visible());

    ws.toggle_sidebar();
    assert!(!ws.is_sidebar_visible());

    ws.toggle_sidebar();
    assert!(ws.is_sidebar_visible());

    ws.set_sidebar_visible(false);
    assert!(!ws.is_sidebar_visible());
}

#[test]
fn test_workspace_sidebar_width() {
    let mut ws = Workspace::new();

    ws.set_sidebar_width(250.0);
    assert_eq!(ws.sidebar_width(), 250.0);

    // Allows small values (icon rail only = 56px)
    ws.set_sidebar_width(56.0);
    assert_eq!(ws.sidebar_width(), 56.0);

    // Clamp to min 0
    ws.set_sidebar_width(-10.0);
    assert_eq!(ws.sidebar_width(), 0.0);

    // Allows large values
    ws.set_sidebar_width(500.0);
    assert_eq!(ws.sidebar_width(), 500.0);
}

#[test]
fn test_workspace_theme() {
    let mut ws = Workspace::new();

    let theme = CodirigentTheme::light();
    ws.set_theme(theme.clone());
    assert_eq!(*ws.theme(), theme);
}

#[test]
fn test_workspace_bounds() {
    let mut ws = Workspace::new();

    let new_bounds = Bounds::from_size(1920.0, 1080.0);
    ws.set_bounds(new_bounds);

    assert_eq!(ws.bounds().size.width, 1920.0);
    assert_eq!(ws.bounds().size.height, 1080.0);
}

#[test]
fn test_workspace_grid_bounds_with_sidebar() {
    let mut ws = Workspace::new();
    ws.set_bounds(Bounds::from_size(1000.0, 800.0));
    ws.set_sidebar_width(200.0);

    let grid_bounds = ws.grid_bounds();
    assert_eq!(grid_bounds.origin.x, 200.0);
    // grid_gap=4.0, grid container padding = 4*2 = 8
    assert_eq!(grid_bounds.size.width, 800.0 - 8.0);
}

#[test]
fn test_workspace_grid_bounds_without_sidebar() {
    let mut ws = Workspace::new();
    ws.set_bounds(Bounds::from_size(1000.0, 800.0));
    ws.set_sidebar_visible(false);

    let grid_bounds = ws.grid_bounds();
    assert_eq!(grid_bounds.origin.x, 0.0);
    // grid_gap=4.0, grid container padding = 4*2 = 8
    assert_eq!(grid_bounds.size.width, 1000.0 - 8.0);
}

#[test]
fn test_workspace_sidebar_bounds() {
    let mut ws = Workspace::new();
    ws.set_bounds(Bounds::from_size(1000.0, 800.0));
    ws.set_sidebar_width(200.0);

    let sidebar = ws.sidebar_bounds().unwrap();
    assert_eq!(sidebar.origin.x, 0.0);
    assert_eq!(sidebar.size.width, 200.0);

    ws.set_sidebar_visible(false);
    assert!(ws.sidebar_bounds().is_none());
}

#[test]
fn test_workspace_session_cell_bounds() {
    let mut ws = Workspace::new();
    ws.set_bounds(Bounds::from_size(1000.0, 800.0));
    ws.set_sidebar_visible(false);
    ws.add_session(make_session(1, "Session 1"));

    let bounds = ws.session_cell_bounds(SessionId(1));
    assert!(bounds.is_some());
    assert!(bounds.unwrap().size.width > 0.0);

    assert!(ws.session_cell_bounds(SessionId(99)).is_none());
}

#[test]
fn test_workspace_session_at_point() {
    let mut ws = Workspace::with_profile(LayoutProfile::Grid2x2);
    ws.set_bounds(Bounds::from_size(1000.0, 800.0));
    ws.set_sidebar_visible(false);

    ws.add_session(make_session(1, "S1"));
    ws.add_session(make_session(2, "S2"));

    // Point in first cell
    let id = ws.session_at_point(Point::new(100.0, 100.0));
    assert_eq!(id, Some(SessionId(1)));

    // Point in second cell (right side)
    let id = ws.session_at_point(Point::new(600.0, 100.0));
    assert_eq!(id, Some(SessionId(2)));
}

#[test]
fn test_workspace_cell_info() {
    let mut ws = Workspace::new();
    ws.set_bounds(Bounds::from_size(1000.0, 800.0));
    ws.add_session(make_session(1, "Session 1"));
    ws.add_session(make_session(2, "Session 2"));

    let cells = ws.cell_info();
    assert_eq!(cells.len(), 2);

    assert_eq!(cells[0].session_id, SessionId(1));
    assert!(cells[0].is_focused);
    assert_eq!(cells[0].name, "Session 1");

    assert_eq!(cells[1].session_id, SessionId(2));
    assert!(!cells[1].is_focused);
}

#[test]
fn test_cell_info_fields() {
    let info = CellInfo {
        session_id: SessionId(1),
        index: 0,
        bounds: Bounds::from_size(100.0, 100.0),
        name: "Test".to_string(),
        status: SessionStatus::Working,
        is_focused: true,
    };

    assert_eq!(info.session_id, SessionId(1));
    assert_eq!(info.index, 0);
    assert_eq!(info.name, "Test");
    assert_eq!(info.status, SessionStatus::Working);
    assert!(info.is_focused);
}

#[test]
fn test_workspace_grid_layout() {
    let mut ws = Workspace::with_profile(LayoutProfile::Grid2x2);
    ws.set_bounds(Bounds::from_size(1000.0, 800.0));
    ws.set_sidebar_visible(false);

    let layout = ws.grid_layout();
    assert_eq!(layout.dimensions(), (2, 2));
    assert_eq!(layout.cell_count(), 4);
}

#[test]
fn test_workspace_single_layout_shows_focused_session() {
    // Test that switching to Single layout displays the focused session, not always the first
    let mut ws = Workspace::with_profile(LayoutProfile::Grid2x2);
    ws.set_bounds(Bounds::from_size(1000.0, 800.0));
    ws.add_session(make_session(1, "Session 1"));
    ws.add_session(make_session(2, "Session 2"));
    ws.add_session(make_session(3, "Session 3"));

    // Focus session 2 (middle session)
    assert!(ws.focus_session(SessionId(2)));
    assert_eq!(ws.focused_session_id(), Some(SessionId(2)));

    // Switch to Single layout
    ws.set_layout(LayoutProfile::Single);

    // The focused session should still be session 2
    assert_eq!(ws.focused_session_id(), Some(SessionId(2)));

    // cell_info should return only one cell with the focused session
    let cells = ws.cell_info();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].session_id, SessionId(2));
    assert!(cells[0].is_focused);
    assert_eq!(cells[0].name, "Session 2");
}

#[test]
fn test_workspace_single_layout_focused_session_already_first() {
    // Test that switching to Single when focused session is already first works correctly
    let mut ws = Workspace::with_profile(LayoutProfile::Grid2x2);
    ws.set_bounds(Bounds::from_size(1000.0, 800.0));
    ws.add_session(make_session(1, "Session 1"));
    ws.add_session(make_session(2, "Session 2"));

    // Session 1 is already focused at index 0
    assert_eq!(ws.focused_session_id(), Some(SessionId(1)));

    // Switch to Single layout
    ws.set_layout(LayoutProfile::Single);

    // Should still show session 1
    assert_eq!(ws.focused_session_id(), Some(SessionId(1)));

    let cells = ws.cell_info();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].session_id, SessionId(1));
    assert!(cells[0].is_focused);
}

#[test]
fn test_workspace_single_layout_preserves_order_on_exit() {
    // Test that sessions return to original order after leaving Single layout
    let mut ws = Workspace::with_profile(LayoutProfile::Grid2x2);
    ws.set_bounds(Bounds::from_size(1000.0, 800.0));
    ws.add_session(make_session(1, "Session 1"));
    ws.add_session(make_session(2, "Session 2"));
    ws.add_session(make_session(3, "Session 3"));
    ws.add_session(make_session(4, "Session 4"));

    // Focus session 3 (index 2)
    assert!(ws.focus_session(SessionId(3)));

    // Switch to Single layout
    ws.set_layout(LayoutProfile::Single);

    // Should show only session 3
    let cells = ws.cell_info();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].session_id, SessionId(3));

    // Switch back to Grid2x2
    ws.set_layout(LayoutProfile::Grid2x2);

    // All sessions should be back in original order
    let cells = ws.cell_info();
    assert_eq!(cells.len(), 4);
    assert_eq!(cells[0].session_id, SessionId(1));
    assert_eq!(cells[1].session_id, SessionId(2));
    assert_eq!(cells[2].session_id, SessionId(3));
    assert_eq!(cells[3].session_id, SessionId(4));

    // Session 3 should still be focused
    assert_eq!(ws.focused_session_id(), Some(SessionId(3)));
    assert!(!cells[0].is_focused);
    assert!(!cells[1].is_focused);
    assert!(cells[2].is_focused);
    assert!(!cells[3].is_focused);
}

// --- set_split_tree tests ---

#[test]
fn test_set_split_tree_transfers_sessions() {
    let mut ws = Workspace::with_profile(LayoutProfile::Grid2x2);
    ws.add_session(make_session(1, "Session 1"));
    ws.add_session(make_session(2, "Session 2"));

    // Build a 3-pane tree
    let tree = LayoutNode::from_grid(1, 3);
    ws.set_split_tree(tree);

    assert!(ws.is_split_tree_mode());
    let visible = ws.visible_sessions();
    // Both sessions should be transferred
    assert_eq!(visible.len(), 2);
    assert!(visible.iter().any(|s| s.id == SessionId(1)));
    assert!(visible.iter().any(|s| s.id == SessionId(2)));
}

#[test]
fn test_set_split_tree_preserves_focus() {
    let mut ws = Workspace::with_profile(LayoutProfile::Grid2x2);
    ws.add_session(make_session(1, "S1"));
    ws.add_session(make_session(2, "S2"));
    ws.focus_session(SessionId(2));

    let tree = LayoutNode::from_grid(1, 3);
    ws.set_split_tree(tree);

    assert_eq!(ws.focused_session_id(), Some(SessionId(2)));
}

#[test]
fn test_set_split_tree_from_grid_state() {
    let mut ws = Workspace::with_profile(LayoutProfile::Grid3x3);
    for i in 1..=4 {
        ws.add_session(make_session(i, &format!("S{}", i)));
    }

    assert!(!ws.is_split_tree_mode());

    // Switch to split tree
    let tree = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf { slot: SlotId(0) }),
        second: Box::new(LayoutNode::Split {
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf { slot: SlotId(1) }),
            second: Box::new(LayoutNode::Leaf { slot: SlotId(2) }),
        }),
    };
    ws.set_split_tree(tree);

    assert!(ws.is_split_tree_mode());
    // 3 slots, 4 sessions — only first 3 get assigned
    assert_eq!(ws.visible_sessions().len(), 3);
}

#[test]
fn test_set_split_tree_from_existing_split() {
    let mut ws = Workspace::new();
    ws.add_session(make_session(1, "S1"));
    ws.add_session(make_session(2, "S2"));

    // First set a simple split tree
    let tree1 = LayoutNode::from_grid(1, 2);
    ws.set_split_tree(tree1);
    assert!(ws.is_split_tree_mode());
    assert_eq!(ws.visible_sessions().len(), 2);

    // Now switch to a different split tree
    let tree2 = LayoutNode::from_grid(2, 2);
    ws.set_split_tree(tree2);
    assert!(ws.is_split_tree_mode());
    assert_eq!(ws.visible_sessions().len(), 2);
}

#[test]
fn test_add_session_to_slot() {
    let mut ws = Workspace::new();
    let tree = LayoutNode::from_grid(1, 3); // 3 slots
    ws.set_split_tree(tree);

    // Get the slot IDs
    let slots: Vec<SlotId> = ws
        .layout_state()
        .as_split_tree()
        .unwrap()
        .assignments()
        .iter()
        .map(|(s, _)| *s)
        .collect();

    // Add session to the second slot specifically
    let session = make_session(1, "Session 1");
    assert!(ws.add_session_to_slot(session, slots[1]));
    assert_eq!(ws.sessions().len(), 1);
    assert_eq!(ws.focused_session_id(), Some(SessionId(1)));

    // The session should be in the second slot, not the first
    let split = ws.layout_state().as_split_tree().unwrap();
    assert_eq!(split.assignments()[0].1, None);
    assert_eq!(split.assignments()[1].1, Some(SessionId(1)));
    assert_eq!(split.assignments()[2].1, None);
}

#[test]
fn test_add_session_to_slot_duplicate_rejected() {
    let mut ws = Workspace::new();
    let tree = LayoutNode::from_grid(1, 3);
    ws.set_split_tree(tree);

    let slots: Vec<SlotId> = ws
        .layout_state()
        .as_split_tree()
        .unwrap()
        .assignments()
        .iter()
        .map(|(s, _)| *s)
        .collect();

    assert!(ws.add_session_to_slot(make_session(1, "S1"), slots[0]));
    // Duplicate session ID should fail
    assert!(!ws.add_session_to_slot(make_session(1, "S1 again"), slots[1]));
    assert_eq!(ws.sessions().len(), 1);
}

#[test]
fn test_close_pane_removes_session_from_workspace() {
    let mut ws = Workspace::new();
    let tree = LayoutNode::from_grid(1, 2); // 2 slots
    ws.set_split_tree(tree);

    ws.add_session(make_session(1, "S1"));
    ws.add_session(make_session(2, "S2"));
    assert_eq!(ws.sessions().len(), 2);

    // Focus session 1, then close its pane
    ws.focus_session(SessionId(1));
    let closed_id = ws.focused_session_id();
    assert_eq!(closed_id, Some(SessionId(1)));

    // close_pane removes the slot from the tree
    assert!(ws.close_pane());

    // Now remove the session as handle_close_pane would
    if let Some(id) = closed_id {
        ws.remove_session(id);
    }
    assert_eq!(ws.sessions().len(), 1);
    assert!(ws.session(SessionId(1)).is_none());
    assert!(ws.session(SessionId(2)).is_some());
}
