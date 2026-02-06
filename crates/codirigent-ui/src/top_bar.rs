//! Unified top bar component.
//!
//! Combines logo, layout tabs, broadcast toggle, token counter,
//! and right-panel toggle into a single 48px bar.

use crate::layout::LayoutProfile;

/// Events emitted by the top bar.
#[derive(Debug, Clone)]
pub enum TopBarEvent {
    /// Layout tab clicked.
    LayoutSelected(LayoutProfile),
    /// Custom layout picker requested.
    CustomLayoutRequested,
    /// Broadcast mode toggled.
    BroadcastToggled(bool),
    /// Right panel (task board) toggle requested.
    RightPanelToggled,
    /// New session requested.
    NewSessionRequested,
}

/// Layout tab button state.
#[derive(Debug, Clone)]
pub struct LayoutTab {
    /// Layout profile this tab represents.
    pub profile: LayoutProfile,
    /// Display label (e.g., "2x2", "3x2").
    pub label: String,
    /// Whether this tab is currently active.
    pub is_active: bool,
}

/// Unified top bar state.
#[derive(Debug)]
pub struct TopBar {
    /// Currently active layout.
    active_layout: LayoutProfile,
    /// Available layout tabs.
    tabs: Vec<LayoutTab>,
    /// Whether broadcast mode is on.
    broadcast_enabled: bool,
    /// Whether the right panel (task board) is visible.
    right_panel_open: bool,
    /// Aggregate token count across sessions (display string).
    token_count: String,
    /// Pending events to be consumed by WorkspaceView.
    pending_events: Vec<TopBarEvent>,
}

impl TopBar {
    /// Top bar height in pixels.
    pub const HEIGHT: f32 = 48.0;

    /// Logo text displayed in top bar.
    pub const LOGO_TEXT: &'static str = "CODIRIGENT";

    /// Create a new top bar with default state.
    pub fn new() -> Self {
        let active = LayoutProfile::default();
        Self {
            tabs: Self::create_tabs(active),
            active_layout: active,
            broadcast_enabled: false,
            right_panel_open: true,
            token_count: "0 tokens".to_string(),
            pending_events: Vec::new(),
        }
    }

    /// Build the list of layout tabs, marking the active one.
    fn create_tabs(active: LayoutProfile) -> Vec<LayoutTab> {
        vec![
            LayoutTab {
                profile: LayoutProfile::Grid2x2,
                label: "2x2".to_string(),
                is_active: active == LayoutProfile::Grid2x2,
            },
            LayoutTab {
                profile: LayoutProfile::Grid2x3,
                label: "3x2".to_string(),
                is_active: active == LayoutProfile::Grid2x3,
            },
            LayoutTab {
                profile: LayoutProfile::Single,
                label: "Focus".to_string(),
                is_active: active == LayoutProfile::Single,
            },
        ]
    }

    /// Get the currently active layout.
    pub fn active_layout(&self) -> LayoutProfile {
        self.active_layout
    }

    /// Set the active layout and refresh tabs.
    pub fn set_active_layout(&mut self, profile: LayoutProfile) {
        self.active_layout = profile;
        self.tabs = Self::create_tabs(profile);
    }

    /// Click a layout tab by index.
    pub fn click_tab(&mut self, index: usize) {
        if let Some(tab) = self.tabs.get(index) {
            let profile = tab.profile;
            self.set_active_layout(profile);
            self.pending_events
                .push(TopBarEvent::LayoutSelected(profile));
        }
    }

    /// Toggle broadcast mode.
    pub fn toggle_broadcast(&mut self) {
        self.broadcast_enabled = !self.broadcast_enabled;
        self.pending_events
            .push(TopBarEvent::BroadcastToggled(self.broadcast_enabled));
    }

    /// Check if broadcast mode is enabled.
    pub fn is_broadcast_enabled(&self) -> bool {
        self.broadcast_enabled
    }

    /// Toggle right panel visibility.
    pub fn toggle_right_panel(&mut self) {
        self.right_panel_open = !self.right_panel_open;
        self.pending_events.push(TopBarEvent::RightPanelToggled);
    }

    /// Check if right panel is open.
    pub fn is_right_panel_open(&self) -> bool {
        self.right_panel_open
    }

    /// Set the token count display string.
    pub fn set_token_count(&mut self, count: String) {
        self.token_count = count;
    }

    /// Get the token count display string.
    pub fn token_count(&self) -> &str {
        &self.token_count
    }

    /// Get the layout tabs.
    pub fn tabs(&self) -> &[LayoutTab] {
        &self.tabs
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<TopBarEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

impl Default for TopBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_top_bar_has_default_layout() {
        let bar = TopBar::new();
        assert_eq!(bar.active_layout(), LayoutProfile::default());
        assert_eq!(bar.tabs().len(), 3);
        assert!(!bar.is_broadcast_enabled());
        assert!(bar.is_right_panel_open());
    }

    #[test]
    fn click_tab_changes_layout() {
        let mut bar = TopBar::new();
        bar.click_tab(1); // 3x2
        assert_eq!(bar.active_layout(), LayoutProfile::Grid2x3);
        let events = bar.drain_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            TopBarEvent::LayoutSelected(LayoutProfile::Grid2x3)
        ));
    }

    #[test]
    fn click_tab_out_of_bounds_is_noop() {
        let mut bar = TopBar::new();
        bar.click_tab(99);
        assert_eq!(bar.drain_events().len(), 0);
    }

    #[test]
    fn toggle_broadcast() {
        let mut bar = TopBar::new();
        assert!(!bar.is_broadcast_enabled());
        bar.toggle_broadcast();
        assert!(bar.is_broadcast_enabled());
        bar.toggle_broadcast();
        assert!(!bar.is_broadcast_enabled());
        let events = bar.drain_events();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn toggle_right_panel() {
        let mut bar = TopBar::new();
        assert!(bar.is_right_panel_open());
        bar.toggle_right_panel();
        assert!(!bar.is_right_panel_open());
        bar.toggle_right_panel();
        assert!(bar.is_right_panel_open());
    }

    #[test]
    fn set_token_count() {
        let mut bar = TopBar::new();
        bar.set_token_count("2.4M tokens".to_string());
        assert_eq!(bar.token_count(), "2.4M tokens");
    }

    #[test]
    fn drain_events_clears() {
        let mut bar = TopBar::new();
        bar.toggle_broadcast();
        bar.toggle_right_panel();
        assert_eq!(bar.drain_events().len(), 2);
        assert_eq!(bar.drain_events().len(), 0);
    }

    #[test]
    fn set_active_layout_updates_tabs() {
        let mut bar = TopBar::new();
        bar.set_active_layout(LayoutProfile::Single);
        assert_eq!(bar.active_layout(), LayoutProfile::Single);
        assert!(bar
            .tabs()
            .iter()
            .find(|t| t.profile == LayoutProfile::Single)
            .unwrap()
            .is_active);
        assert!(!bar
            .tabs()
            .iter()
            .find(|t| t.profile == LayoutProfile::Grid2x2)
            .unwrap()
            .is_active);
    }

    #[test]
    fn default_is_same_as_new() {
        let default = TopBar::default();
        let new = TopBar::new();
        assert_eq!(default.active_layout(), new.active_layout());
        assert_eq!(default.is_broadcast_enabled(), new.is_broadcast_enabled());
    }
}
