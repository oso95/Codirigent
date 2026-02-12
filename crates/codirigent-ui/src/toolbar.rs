//! Sessions toolbar component.
//!
//! Provides the toolbar above the sessions grid with layout tabs,
//! broadcast button, and new session button.

use crate::layout::LayoutProfile;
use codirigent_core::{LayoutNode, SlotId, SplitDirection};

/// Events emitted by the sessions toolbar.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolbarEvent {
    /// Layout tab was clicked.
    LayoutSelected(LayoutProfile),
    /// Custom layout was requested with specific dimensions.
    CustomLayoutRequested {
        /// Number of rows.
        rows: u32,
        /// Number of columns.
        cols: u32,
    },
    /// Broadcast button was toggled.
    BroadcastToggled(bool),
    /// New session button was clicked.
    NewSessionRequested,
    /// Custom layout picker was opened.
    CustomPickerOpened,
    /// Custom layout picker was closed.
    CustomPickerClosed,
}

/// A layout tab button in the toolbar.
#[derive(Debug, Clone)]
pub struct LayoutTabButton {
    /// The layout profile this tab represents.
    pub profile: LayoutProfile,
    /// Display label for the tab.
    pub label: String,
    /// Whether this tab is currently active.
    pub is_active: bool,
    /// Whether this tab is currently hovered.
    pub is_hovered: bool,
}

impl LayoutTabButton {
    /// Create a new layout tab button.
    pub fn new(profile: LayoutProfile, is_active: bool) -> Self {
        Self {
            label: profile.display_name(),
            profile,
            is_active,
            is_hovered: false,
        }
    }

    /// Create a custom layout tab button.
    pub fn custom(is_active: bool) -> Self {
        Self {
            profile: LayoutProfile::Custom { rows: 2, cols: 2 }, // Default placeholder
            label: "Custom".to_string(),
            is_active,
            is_hovered: false,
        }
    }
}

/// Mode for the custom layout picker modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CustomLayoutMode {
    /// Traditional NxM grid configuration.
    #[default]
    Grid,
    /// Interactive binary split tree builder.
    Split,
}

/// Custom layout picker state.
#[derive(Debug, Clone)]
pub struct CustomLayoutPicker {
    /// Whether the picker is visible.
    pub is_open: bool,
    /// Current rows value in the input.
    pub rows_input: String,
    /// Current columns value in the input.
    pub cols_input: String,
    /// Error message if validation failed.
    pub error: Option<String>,
    /// Which input field is focused (0 = rows, 1 = columns).
    focused_input: Option<usize>,
    /// Current mode (Grid or Split).
    pub mode: CustomLayoutMode,
    /// Draft split tree for the split builder.
    pub split_tree: LayoutNode,
    /// Currently selected slot in the split preview.
    pub selected_slot: Option<SlotId>,
    /// Next slot ID counter for generating unique slot IDs.
    next_slot_id: u32,
}

impl Default for CustomLayoutPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomLayoutPicker {
    /// Create a new custom layout picker.
    pub fn new() -> Self {
        Self {
            is_open: false,
            rows_input: "2".to_string(),
            cols_input: "2".to_string(),
            error: None,
            focused_input: None,
            mode: CustomLayoutMode::Grid,
            split_tree: LayoutNode::Leaf { slot: SlotId(0) },
            selected_slot: Some(SlotId(0)),
            next_slot_id: 1,
        }
    }

    /// Open the picker with default values.
    pub fn open(&mut self) {
        self.is_open = true;
        self.rows_input = "2".to_string();
        self.cols_input = "2".to_string();
        self.error = None;
        self.focused_input = Some(0);
        self.mode = CustomLayoutMode::Grid;
    }

    /// Open the picker with specific values.
    pub fn open_with(&mut self, rows: u32, cols: u32) {
        self.is_open = true;
        self.rows_input = rows.to_string();
        self.cols_input = cols.to_string();
        self.error = None;
        self.focused_input = Some(0);
        self.mode = CustomLayoutMode::Grid;
    }

    /// Open the picker with state derived from the current workspace layout.
    ///
    /// If `current_tree` is `Some`, populates the split builder from it and
    /// sets mode to Split. Otherwise defaults to Grid mode with a single leaf.
    pub fn open_with_state(&mut self, current_tree: Option<LayoutNode>, rows: u32, cols: u32) {
        self.is_open = true;
        self.rows_input = rows.to_string();
        self.cols_input = cols.to_string();
        self.error = None;

        if let Some(tree) = current_tree {
            let max_id = tree.slots_in_order().iter().map(|s| s.0).max().unwrap_or(0);
            let first_slot = tree.slots_in_order().first().copied();
            self.split_tree = tree;
            self.next_slot_id = max_id + 1;
            self.selected_slot = first_slot;
            self.mode = CustomLayoutMode::Split;
            self.focused_input = None;
        } else {
            self.split_tree = LayoutNode::Leaf { slot: SlotId(0) };
            self.selected_slot = Some(SlotId(0));
            self.next_slot_id = 1;
            self.mode = CustomLayoutMode::Grid;
            self.focused_input = Some(0);
        }
    }

    /// Close the picker.
    pub fn close(&mut self) {
        self.is_open = false;
        self.error = None;
        self.focused_input = None;
        // Reset split state
        self.split_tree = LayoutNode::Leaf { slot: SlotId(0) };
        self.selected_slot = Some(SlotId(0));
        self.next_slot_id = 1;
        self.mode = CustomLayoutMode::Grid;
    }

    /// Switch between Grid and Split modes.
    pub fn set_mode(&mut self, mode: CustomLayoutMode) {
        self.mode = mode;
        self.error = None;
    }

    /// Select a slot in the split preview.
    pub fn select_slot(&mut self, slot: SlotId) {
        if self.split_tree.contains_slot(slot) {
            self.selected_slot = Some(slot);
        }
    }

    /// Split the currently selected slot in the given direction.
    ///
    /// Returns `true` if the split was successful.
    pub fn split_selected(&mut self, direction: SplitDirection) -> bool {
        let Some(target) = self.selected_slot else {
            return false;
        };
        let new_slot = SlotId(self.next_slot_id);
        if let Some(new_tree) = self.split_tree.split_slot(target, direction, 0.5, new_slot) {
            self.split_tree = new_tree;
            self.next_slot_id += 1;
            self.error = None;
            true
        } else {
            false
        }
    }

    /// Remove the currently selected slot.
    ///
    /// Returns `true` if the removal was successful.
    pub fn remove_selected(&mut self) -> bool {
        let Some(target) = self.selected_slot else {
            return false;
        };
        if self.split_tree.leaf_count() <= 1 {
            self.error = Some("Cannot remove the last pane".to_string());
            return false;
        }
        if let Some(new_tree) = self.split_tree.close_slot(target) {
            self.split_tree = new_tree;
            // Select the first remaining slot
            self.selected_slot = self.split_tree.slots_in_order().first().copied();
            self.error = None;
            true
        } else {
            false
        }
    }

    /// Validate the split tree and return it if valid.
    ///
    /// Ensures the tree has between 1 and 20 panes.
    pub fn validate_split(&mut self) -> Option<LayoutNode> {
        let count = self.split_tree.leaf_count();
        if count < 1 {
            self.error = Some("Must have at least 1 pane".to_string());
            return None;
        }
        if count > 20 {
            self.error = Some("Maximum 20 panes allowed".to_string());
            return None;
        }
        self.error = None;
        Some(self.split_tree.clone())
    }

    /// Validate and parse the current input values.
    ///
    /// Returns `Some((rows, cols))` if valid, or `None` if invalid.
    /// Sets the error message on failure.
    pub fn validate(&mut self) -> Option<(u32, u32)> {
        let rows = match self.rows_input.parse::<u32>() {
            Ok(r) if (1..=10).contains(&r) => r,
            Ok(_) => {
                self.error = Some("Rows must be between 1 and 10".to_string());
                return None;
            }
            Err(_) => {
                self.error = Some("Invalid rows value".to_string());
                return None;
            }
        };

        let cols = match self.cols_input.parse::<u32>() {
            Ok(c) if (1..=10).contains(&c) => c,
            Ok(_) => {
                self.error = Some("Columns must be between 1 and 10".to_string());
                return None;
            }
            Err(_) => {
                self.error = Some("Invalid columns value".to_string());
                return None;
            }
        };

        self.error = None;
        Some((rows, cols))
    }

    /// Update the rows input value.
    pub fn set_rows(&mut self, value: String) {
        self.rows_input = value;
    }

    /// Update the columns input value.
    pub fn set_cols(&mut self, value: String) {
        self.cols_input = value;
    }

    /// Set focus to a specific input field.
    pub fn set_focus(&mut self, field: usize) {
        self.focused_input = Some(field);
    }

    /// Get the currently focused input field.
    pub fn focused_input(&self) -> Option<usize> {
        self.focused_input
    }

    /// Handle a character input.
    pub fn handle_char_input(&mut self, c: char) {
        match self.focused_input {
            Some(0) => self.rows_input.push(c),
            Some(1) => self.cols_input.push(c),
            _ => {}
        }
    }

    /// Handle backspace for the focused input.
    pub fn handle_backspace(&mut self) {
        match self.focused_input {
            Some(0) => {
                self.rows_input.pop();
            }
            Some(1) => {
                self.cols_input.pop();
            }
            _ => {}
        }
    }
}

/// Sessions toolbar state.
#[derive(Debug)]
pub struct SessionsToolbar {
    /// Currently active layout profile.
    active_layout: LayoutProfile,
    /// Available layout tabs.
    tabs: Vec<LayoutTabButton>,
    /// Whether broadcast mode is enabled.
    broadcast_enabled: bool,
    /// Custom layout picker state.
    custom_picker: CustomLayoutPicker,
    /// Pending events.
    pending_events: Vec<ToolbarEvent>,
    /// Toolbar height in pixels.
    height: f32,
}

impl Default for SessionsToolbar {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionsToolbar {
    /// Default toolbar height.
    pub const DEFAULT_HEIGHT: f32 = 48.0;

    /// Create a new sessions toolbar.
    pub fn new() -> Self {
        let active_layout = LayoutProfile::default();
        Self {
            tabs: Self::create_tabs(active_layout),
            active_layout,
            broadcast_enabled: false,
            custom_picker: CustomLayoutPicker::new(),
            pending_events: Vec::new(),
            height: Self::DEFAULT_HEIGHT,
        }
    }

    /// Create tabs for the toolbar.
    fn create_tabs(active: LayoutProfile) -> Vec<LayoutTabButton> {
        vec![
            LayoutTabButton::new(LayoutProfile::Grid2x2, active == LayoutProfile::Grid2x2),
            LayoutTabButton::new(LayoutProfile::Grid2x3, active == LayoutProfile::Grid2x3),
            LayoutTabButton::new(LayoutProfile::Grid3x3, active == LayoutProfile::Grid3x3),
            LayoutTabButton::custom(active.is_custom()),
        ]
    }

    /// Get the active layout profile.
    pub fn active_layout(&self) -> LayoutProfile {
        self.active_layout
    }

    /// Set the active layout profile.
    pub fn set_active_layout(&mut self, profile: LayoutProfile) {
        self.active_layout = profile;
        self.tabs = Self::create_tabs(profile);
    }

    /// Get the layout tabs.
    pub fn tabs(&self) -> &[LayoutTabButton] {
        &self.tabs
    }

    /// Get mutable access to tabs (for hover state).
    pub fn tabs_mut(&mut self) -> &mut [LayoutTabButton] {
        &mut self.tabs
    }

    /// Click a layout tab.
    pub fn click_tab(&mut self, index: usize) {
        if let Some(tab) = self.tabs.get(index) {
            if tab.label == "Custom" {
                // Open custom picker instead of selecting directly
                self.custom_picker.open();
                self.pending_events.push(ToolbarEvent::CustomPickerOpened);
            } else {
                let profile = tab.profile;
                self.set_active_layout(profile);
                self.pending_events
                    .push(ToolbarEvent::LayoutSelected(profile));
            }
        }
    }

    /// Select a layout directly.
    pub fn select_layout(&mut self, profile: LayoutProfile) {
        self.set_active_layout(profile);
        self.pending_events
            .push(ToolbarEvent::LayoutSelected(profile));
    }

    /// Check if broadcast mode is enabled.
    pub fn is_broadcast_enabled(&self) -> bool {
        self.broadcast_enabled
    }

    /// Toggle broadcast mode.
    pub fn toggle_broadcast(&mut self) {
        self.broadcast_enabled = !self.broadcast_enabled;
        self.pending_events
            .push(ToolbarEvent::BroadcastToggled(self.broadcast_enabled));
    }

    /// Set broadcast mode.
    pub fn set_broadcast(&mut self, enabled: bool) {
        if self.broadcast_enabled != enabled {
            self.broadcast_enabled = enabled;
            self.pending_events
                .push(ToolbarEvent::BroadcastToggled(enabled));
        }
    }

    /// Request a new session.
    pub fn request_new_session(&mut self) {
        self.pending_events.push(ToolbarEvent::NewSessionRequested);
    }

    /// Get the custom layout picker.
    pub fn custom_picker(&self) -> &CustomLayoutPicker {
        &self.custom_picker
    }

    /// Get mutable access to the custom layout picker.
    pub fn custom_picker_mut(&mut self) -> &mut CustomLayoutPicker {
        &mut self.custom_picker
    }

    /// Open the custom layout picker.
    pub fn open_custom_picker(&mut self) {
        if let LayoutProfile::Custom { rows, cols } = self.active_layout {
            self.custom_picker.open_with(rows, cols);
        } else {
            self.custom_picker.open();
        }
        self.pending_events.push(ToolbarEvent::CustomPickerOpened);
    }

    /// Close the custom layout picker.
    pub fn close_custom_picker(&mut self) {
        self.custom_picker.close();
        self.pending_events.push(ToolbarEvent::CustomPickerClosed);
    }

    /// Submit the custom layout picker.
    ///
    /// Validates the input and emits the appropriate event.
    pub fn submit_custom_layout(&mut self) -> bool {
        if let Some((rows, cols)) = self.custom_picker.validate() {
            let profile = LayoutProfile::Custom { rows, cols };
            self.set_active_layout(profile);
            self.custom_picker.close();
            self.pending_events
                .push(ToolbarEvent::CustomLayoutRequested { rows, cols });
            true
        } else {
            false
        }
    }

    /// Get the toolbar height.
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Set the toolbar height.
    pub fn set_height(&mut self, height: f32) {
        self.height = height.max(32.0);
    }

    /// Take all pending events.
    pub fn take_events(&mut self) -> Vec<ToolbarEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

/// Rendering hints for the toolbar.
#[derive(Debug, Clone)]
pub struct ToolbarRenderHints {
    /// Layout tab buttons.
    pub tabs: Vec<LayoutTabButton>,
    /// Whether broadcast is enabled.
    pub broadcast_enabled: bool,
    /// Whether custom picker is open.
    pub custom_picker_open: bool,
    /// Custom picker rows input.
    pub custom_rows: String,
    /// Custom picker cols input.
    pub custom_cols: String,
    /// Custom picker error message.
    pub custom_error: Option<String>,
    /// Toolbar height.
    pub height: f32,
}

impl SessionsToolbar {
    /// Generate rendering hints for the toolbar.
    pub fn render_hints(&self) -> ToolbarRenderHints {
        ToolbarRenderHints {
            tabs: self.tabs.clone(),
            broadcast_enabled: self.broadcast_enabled,
            custom_picker_open: self.custom_picker.is_open,
            custom_rows: self.custom_picker.rows_input.clone(),
            custom_cols: self.custom_picker.cols_input.clone(),
            custom_error: self.custom_picker.error.clone(),
            height: self.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toolbar_new() {
        let toolbar = SessionsToolbar::new();
        assert_eq!(toolbar.active_layout(), LayoutProfile::default());
        assert!(!toolbar.is_broadcast_enabled());
        assert!(!toolbar.custom_picker().is_open);
    }

    #[test]
    fn test_toolbar_default() {
        let toolbar = SessionsToolbar::default();
        assert_eq!(toolbar.active_layout(), LayoutProfile::default());
    }

    #[test]
    fn test_toolbar_tabs() {
        let toolbar = SessionsToolbar::new();
        let tabs = toolbar.tabs();
        assert_eq!(tabs.len(), 4);
        assert_eq!(tabs[0].label, "2x2");
        assert_eq!(tabs[1].label, "2x3");
        assert_eq!(tabs[2].label, "3x3");
        assert_eq!(tabs[3].label, "Custom");
    }

    #[test]
    fn test_click_layout_tab() {
        let mut toolbar = SessionsToolbar::new();
        toolbar.click_tab(1); // Click 2x3
        assert_eq!(toolbar.active_layout(), LayoutProfile::Grid2x3);

        let events = toolbar.take_events();
        assert!(matches!(
            &events[0],
            ToolbarEvent::LayoutSelected(LayoutProfile::Grid2x3)
        ));
    }

    #[test]
    fn test_click_custom_tab() {
        let mut toolbar = SessionsToolbar::new();
        toolbar.click_tab(3); // Click Custom
        assert!(toolbar.custom_picker().is_open);

        let events = toolbar.take_events();
        assert!(matches!(&events[0], ToolbarEvent::CustomPickerOpened));
    }

    #[test]
    fn test_toggle_broadcast() {
        let mut toolbar = SessionsToolbar::new();
        assert!(!toolbar.is_broadcast_enabled());

        toolbar.toggle_broadcast();
        assert!(toolbar.is_broadcast_enabled());

        let events = toolbar.take_events();
        assert!(matches!(&events[0], ToolbarEvent::BroadcastToggled(true)));

        toolbar.toggle_broadcast();
        assert!(!toolbar.is_broadcast_enabled());
    }

    #[test]
    fn test_request_new_session() {
        let mut toolbar = SessionsToolbar::new();
        toolbar.request_new_session();

        let events = toolbar.take_events();
        assert!(matches!(&events[0], ToolbarEvent::NewSessionRequested));
    }

    #[test]
    fn test_custom_picker_validate_valid() {
        let mut picker = CustomLayoutPicker::new();
        picker.rows_input = "3".to_string();
        picker.cols_input = "4".to_string();

        let result = picker.validate();
        assert_eq!(result, Some((3, 4)));
        assert!(picker.error.is_none());
    }

    #[test]
    fn test_custom_picker_validate_invalid_rows() {
        let mut picker = CustomLayoutPicker::new();
        picker.rows_input = "15".to_string(); // Too big
        picker.cols_input = "2".to_string();

        let result = picker.validate();
        assert!(result.is_none());
        assert!(picker.error.is_some());
    }

    #[test]
    fn test_custom_picker_validate_invalid_cols() {
        let mut picker = CustomLayoutPicker::new();
        picker.rows_input = "2".to_string();
        picker.cols_input = "abc".to_string(); // Invalid

        let result = picker.validate();
        assert!(result.is_none());
        assert!(picker.error.is_some());
    }

    #[test]
    fn test_custom_picker_validate_zero() {
        let mut picker = CustomLayoutPicker::new();
        picker.rows_input = "0".to_string(); // Too small
        picker.cols_input = "2".to_string();

        let result = picker.validate();
        assert!(result.is_none());
        assert!(picker.error.is_some());
    }

    #[test]
    fn test_submit_custom_layout() {
        let mut toolbar = SessionsToolbar::new();
        toolbar.open_custom_picker();
        toolbar.custom_picker_mut().rows_input = "4".to_string();
        toolbar.custom_picker_mut().cols_input = "3".to_string();

        let _ = toolbar.take_events(); // Clear open event

        let success = toolbar.submit_custom_layout();
        assert!(success);
        assert!(!toolbar.custom_picker().is_open);
        assert_eq!(
            toolbar.active_layout(),
            LayoutProfile::Custom { rows: 4, cols: 3 }
        );

        let events = toolbar.take_events();
        assert!(matches!(
            &events[0],
            ToolbarEvent::CustomLayoutRequested { rows: 4, cols: 3 }
        ));
    }

    #[test]
    fn test_submit_invalid_custom_layout() {
        let mut toolbar = SessionsToolbar::new();
        toolbar.open_custom_picker();
        toolbar.custom_picker_mut().rows_input = "invalid".to_string();

        let success = toolbar.submit_custom_layout();
        assert!(!success);
        assert!(toolbar.custom_picker().is_open); // Still open
    }

    #[test]
    fn test_close_custom_picker() {
        let mut toolbar = SessionsToolbar::new();
        toolbar.open_custom_picker();
        toolbar.close_custom_picker();
        assert!(!toolbar.custom_picker().is_open);
    }

    #[test]
    fn test_open_custom_picker_with_current() {
        let mut toolbar = SessionsToolbar::new();
        toolbar.set_active_layout(LayoutProfile::Custom { rows: 5, cols: 6 });

        toolbar.open_custom_picker();
        assert_eq!(toolbar.custom_picker().rows_input, "5");
        assert_eq!(toolbar.custom_picker().cols_input, "6");
    }

    #[test]
    fn test_select_layout_direct() {
        let mut toolbar = SessionsToolbar::new();
        toolbar.select_layout(LayoutProfile::Grid3x3);
        assert_eq!(toolbar.active_layout(), LayoutProfile::Grid3x3);
    }

    #[test]
    fn test_set_broadcast() {
        let mut toolbar = SessionsToolbar::new();
        toolbar.set_broadcast(true);
        assert!(toolbar.is_broadcast_enabled());

        // Setting to same value should not emit event
        toolbar.take_events();
        toolbar.set_broadcast(true);
        assert!(toolbar.take_events().is_empty());
    }

    #[test]
    fn test_toolbar_height() {
        let mut toolbar = SessionsToolbar::new();
        assert_eq!(toolbar.height(), SessionsToolbar::DEFAULT_HEIGHT);

        toolbar.set_height(60.0);
        assert_eq!(toolbar.height(), 60.0);

        toolbar.set_height(10.0); // Too small
        assert!(toolbar.height() >= 32.0);
    }

    #[test]
    fn test_render_hints() {
        let mut toolbar = SessionsToolbar::new();
        toolbar.toggle_broadcast();
        toolbar.open_custom_picker();
        toolbar.custom_picker_mut().rows_input = "5".to_string();

        let hints = toolbar.render_hints();
        assert!(hints.broadcast_enabled);
        assert!(hints.custom_picker_open);
        assert_eq!(hints.custom_rows, "5");
    }

    #[test]
    fn test_layout_tab_button_new() {
        let tab = LayoutTabButton::new(LayoutProfile::Grid2x2, true);
        assert_eq!(tab.label, "2x2");
        assert!(tab.is_active);
        assert!(!tab.is_hovered);
    }

    #[test]
    fn test_layout_tab_button_custom() {
        let tab = LayoutTabButton::custom(false);
        assert_eq!(tab.label, "Custom");
        assert!(!tab.is_active);
    }

    #[test]
    fn test_tabs_mut() {
        let mut toolbar = SessionsToolbar::new();
        let tabs = toolbar.tabs_mut();
        tabs[0].is_hovered = true;
        assert!(toolbar.tabs()[0].is_hovered);
    }

    #[test]
    fn test_custom_picker_set_values() {
        let mut picker = CustomLayoutPicker::new();
        picker.set_rows("7".to_string());
        picker.set_cols("8".to_string());
        assert_eq!(picker.rows_input, "7");
        assert_eq!(picker.cols_input, "8");
    }

    #[test]
    fn test_custom_picker_focus_and_input() {
        let mut picker = CustomLayoutPicker::new();
        picker.open();
        assert_eq!(picker.focused_input(), Some(0));

        picker.handle_char_input('3');
        assert!(picker.rows_input.ends_with('3'));

        picker.set_focus(1);
        picker.handle_char_input('4');
        assert!(picker.cols_input.ends_with('4'));

        picker.handle_backspace();
        assert!(!picker.cols_input.ends_with('4'));
    }

    #[test]
    fn test_active_tab_updates() {
        let mut toolbar = SessionsToolbar::new();
        assert!(toolbar.tabs()[0].is_active); // 2x2 is default

        toolbar.select_layout(LayoutProfile::Grid2x3);
        assert!(!toolbar.tabs()[0].is_active);
        assert!(toolbar.tabs()[1].is_active); // 2x3 should now be active
    }

    // --- Split builder tests ---

    #[test]
    fn test_split_selected_creates_correct_tree() {
        let mut picker = CustomLayoutPicker::new();
        picker.mode = CustomLayoutMode::Split;
        // Starts as single leaf SlotId(0)
        assert_eq!(picker.split_tree.leaf_count(), 1);

        // Split horizontally
        assert!(picker.split_selected(SplitDirection::Horizontal));
        assert_eq!(picker.split_tree.leaf_count(), 2);

        // The tree should be a horizontal split with slots 0 and 1
        let slots = picker.split_tree.slots_in_order();
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0], SlotId(0));
        assert_eq!(slots[1], SlotId(1));

        // Split the first slot vertically
        picker.selected_slot = Some(SlotId(0));
        assert!(picker.split_selected(SplitDirection::Vertical));
        assert_eq!(picker.split_tree.leaf_count(), 3);

        let slots = picker.split_tree.slots_in_order();
        assert_eq!(slots.len(), 3);
        assert!(slots.contains(&SlotId(0)));
        assert!(slots.contains(&SlotId(1)));
        assert!(slots.contains(&SlotId(2)));
    }

    #[test]
    fn test_remove_selected_promotes_sibling() {
        let mut picker = CustomLayoutPicker::new();
        // Create 2 panes
        picker.split_selected(SplitDirection::Horizontal);
        assert_eq!(picker.split_tree.leaf_count(), 2);

        // Select and remove slot 1
        picker.selected_slot = Some(SlotId(1));
        assert!(picker.remove_selected());
        assert_eq!(picker.split_tree.leaf_count(), 1);

        // Selected should move to remaining slot
        assert!(picker.selected_slot.is_some());
        assert!(picker
            .split_tree
            .contains_slot(picker.selected_slot.unwrap()));
    }

    #[test]
    fn test_remove_selected_cannot_remove_last_pane() {
        let mut picker = CustomLayoutPicker::new();
        // Single pane
        assert_eq!(picker.split_tree.leaf_count(), 1);

        picker.selected_slot = Some(SlotId(0));
        assert!(!picker.remove_selected());
        assert!(picker.error.is_some());
        assert_eq!(picker.split_tree.leaf_count(), 1);
    }

    #[test]
    fn test_validate_split_enforces_limits() {
        let mut picker = CustomLayoutPicker::new();

        // Single pane is valid
        let result = picker.validate_split();
        assert!(result.is_some());
        assert!(picker.error.is_none());

        // Build 20 panes (split 19 more times)
        for _ in 0..19 {
            let slots = picker.split_tree.slots_in_order();
            picker.selected_slot = Some(slots[0]);
            picker.split_selected(SplitDirection::Horizontal);
        }
        assert_eq!(picker.split_tree.leaf_count(), 20);
        assert!(picker.validate_split().is_some());

        // Add one more to exceed limit
        let slots = picker.split_tree.slots_in_order();
        picker.selected_slot = Some(slots[0]);
        picker.split_selected(SplitDirection::Horizontal);
        assert_eq!(picker.split_tree.leaf_count(), 21);
        assert!(picker.validate_split().is_none());
        assert!(picker.error.is_some());
    }

    #[test]
    fn test_select_slot_only_selects_existing() {
        let mut picker = CustomLayoutPicker::new();
        // Single leaf with SlotId(0)

        // Selecting existing slot works
        picker.select_slot(SlotId(0));
        assert_eq!(picker.selected_slot, Some(SlotId(0)));

        // Selecting non-existent slot doesn't change selection
        picker.select_slot(SlotId(99));
        assert_eq!(picker.selected_slot, Some(SlotId(0)));
    }

    #[test]
    fn test_open_with_state_existing_tree() {
        let mut picker = CustomLayoutPicker::new();

        // Build a tree externally
        let tree = LayoutNode::from_grid(2, 2);
        let expected_slots = tree.slots_in_order();
        let max_id = expected_slots.iter().map(|s| s.0).max().unwrap_or(0);

        picker.open_with_state(Some(tree.clone()), 2, 2);

        assert!(picker.is_open);
        assert_eq!(picker.mode, CustomLayoutMode::Split);
        assert_eq!(picker.split_tree, tree);
        assert_eq!(picker.next_slot_id, max_id + 1);
        assert!(picker.selected_slot.is_some());
        assert!(expected_slots.contains(&picker.selected_slot.unwrap()));
    }

    #[test]
    fn test_open_with_state_no_tree() {
        let mut picker = CustomLayoutPicker::new();

        picker.open_with_state(None, 3, 4);

        assert!(picker.is_open);
        assert_eq!(picker.mode, CustomLayoutMode::Grid);
        assert_eq!(picker.rows_input, "3");
        assert_eq!(picker.cols_input, "4");
        assert_eq!(picker.split_tree.leaf_count(), 1);
    }

    #[test]
    fn test_mode_switching_preserves_state() {
        let mut picker = CustomLayoutPicker::new();
        picker.open();

        // Set up grid state
        picker.rows_input = "5".to_string();
        picker.cols_input = "6".to_string();

        // Switch to split and back
        picker.set_mode(CustomLayoutMode::Split);
        assert_eq!(picker.mode, CustomLayoutMode::Split);

        picker.set_mode(CustomLayoutMode::Grid);
        assert_eq!(picker.mode, CustomLayoutMode::Grid);
        // Grid state preserved
        assert_eq!(picker.rows_input, "5");
        assert_eq!(picker.cols_input, "6");
    }

    #[test]
    fn test_close_resets_split_state() {
        let mut picker = CustomLayoutPicker::new();
        picker.open();
        picker.set_mode(CustomLayoutMode::Split);
        picker.split_selected(SplitDirection::Horizontal);
        assert_eq!(picker.split_tree.leaf_count(), 2);

        picker.close();

        // Split state should be reset
        assert_eq!(picker.split_tree.leaf_count(), 1);
        assert_eq!(picker.next_slot_id, 1);
        assert_eq!(picker.selected_slot, Some(SlotId(0)));
        assert_eq!(picker.mode, CustomLayoutMode::Grid);
    }

    #[test]
    fn test_split_selected_no_selection() {
        let mut picker = CustomLayoutPicker::new();
        picker.selected_slot = None;
        assert!(!picker.split_selected(SplitDirection::Horizontal));
    }
}
