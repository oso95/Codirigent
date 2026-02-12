//! Unified top bar component.
//!
//! Combines logo, layout tabs, broadcast toggle, token counter,
//! and right-panel toggle into a single 48px bar.

use crate::layout::LayoutProfile;
use crate::layout_profile::{LayoutProfileManager, SavedLayoutProfile};
use codirigent_core::LayoutMode;

/// Events emitted by the top bar.
#[derive(Debug, Clone)]
pub enum TopBarEvent {
    /// Layout tab clicked — carries the LayoutMode to apply.
    LayoutSelected(LayoutMode),
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
    /// Profile ID in the manager (used for activation).
    pub profile_id: String,
    /// Display label (e.g., "2x2", "3x2").
    pub label: String,
    /// Whether this tab is currently active.
    pub is_active: bool,
    /// The layout mode this tab represents.
    pub layout_mode: LayoutMode,
    /// Whether this is a user-saved (non-default) profile.
    pub is_user_saved: bool,
}

/// IDs of the built-in default profiles.
const DEFAULT_PROFILE_IDS: &[&str] = &["2x2", "2x3", "single"];

/// Unified top bar state.
#[derive(Debug)]
pub struct TopBar {
    /// Currently active layout.
    active_layout: LayoutProfile,
    /// Profile manager holding all profiles (defaults + user-saved).
    pub profile_manager: LayoutProfileManager,
    /// Available layout tabs (derived from profile manager).
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
        let mut manager = LayoutProfileManager::new();
        // Add only the three profiles matching the original hardcoded tabs
        manager.add_profile(SavedLayoutProfile::new(
            "2x2",
            "2x2",
            LayoutMode::Grid { rows: 2, cols: 2 },
        ));
        manager.add_profile(SavedLayoutProfile::new(
            "2x3",
            "3x2",
            LayoutMode::Grid { rows: 2, cols: 3 },
        ));
        manager.add_profile(SavedLayoutProfile::new(
            "single",
            "Focus",
            LayoutMode::Single,
        ));
        manager.set_active("2x2");

        let tabs = Self::derive_tabs(&manager);
        Self {
            active_layout: LayoutProfile::default(),
            profile_manager: manager,
            tabs,
            broadcast_enabled: false,
            right_panel_open: true,
            token_count: "0 tokens".to_string(),
            pending_events: Vec::new(),
        }
    }

    /// Derive tabs from the profile manager state.
    fn derive_tabs(manager: &LayoutProfileManager) -> Vec<LayoutTab> {
        let active_id = manager.active_id();
        manager
            .list_profiles()
            .iter()
            .map(|p| {
                let is_default = DEFAULT_PROFILE_IDS.contains(&p.id.as_str());
                LayoutTab {
                    profile_id: p.id.clone(),
                    label: p.name.clone(),
                    is_active: active_id == Some(p.id.as_str()),
                    layout_mode: p.layout.clone(),
                    is_user_saved: !is_default,
                }
            })
            .collect()
    }

    /// Refresh tabs from the current profile manager state.
    fn refresh_tabs(&mut self) {
        self.tabs = Self::derive_tabs(&self.profile_manager);
    }

    /// Get the currently active layout.
    pub fn active_layout(&self) -> LayoutProfile {
        self.active_layout
    }

    /// Set the active layout and refresh tabs.
    pub fn set_active_layout(&mut self, profile: LayoutProfile) {
        self.active_layout = profile;
        // Try to find a matching profile in the manager
        let mode = profile.to_mode();
        let matching_id = self
            .profile_manager
            .list_profiles()
            .iter()
            .find(|p| p.layout == mode)
            .map(|p| p.id.clone());
        if let Some(id) = matching_id {
            self.profile_manager.set_active(&id);
        } else {
            // No matching profile — deactivate all
            // Use an impossible ID so nothing matches
            self.profile_manager.set_active("__none__");
        }
        self.refresh_tabs();
    }

    /// Set the active profile by its manager ID and refresh tabs.
    pub fn set_active_profile_id(&mut self, id: &str) {
        self.profile_manager.set_active(id);
        self.refresh_tabs();
    }

    /// Click a layout tab by index.
    pub fn click_tab(&mut self, index: usize) {
        if let Some(tab) = self.tabs.get(index) {
            let layout_mode = tab.layout_mode.clone();
            let profile_id = tab.profile_id.clone();
            self.profile_manager.set_active(&profile_id);
            self.refresh_tabs();
            self.pending_events
                .push(TopBarEvent::LayoutSelected(layout_mode));
        }
    }

    /// Remove a user-saved profile by index and refresh tabs.
    pub fn remove_tab(&mut self, index: usize) {
        if let Some(tab) = self.tabs.get(index) {
            if tab.is_user_saved {
                let id = tab.profile_id.clone();
                self.profile_manager.remove_profile(&id);
                self.refresh_tabs();
            }
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

    /// Request opening the custom layout picker.
    pub fn request_custom_layout(&mut self) {
        self.pending_events.push(TopBarEvent::CustomLayoutRequested);
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

    /// Load saved profiles from UserSettings and add them to the profile manager.
    pub fn load_saved_profiles(&mut self, saved_layouts: Vec<codirigent_core::SavedLayout>) {
        for saved in saved_layouts {
            // Skip if this is a built-in profile (already added in new())
            if DEFAULT_PROFILE_IDS.contains(&saved.id.as_str()) {
                continue;
            }
            let profile = SavedLayoutProfile::from_saved_layout(saved);
            self.profile_manager.add_profile(profile);
        }
        self.refresh_tabs();
    }

    /// Export user-saved profiles (non-default) for persistence.
    pub fn export_user_profiles(&self) -> Vec<codirigent_core::SavedLayout> {
        self.profile_manager
            .list_profiles()
            .iter()
            .filter(|p| !DEFAULT_PROFILE_IDS.contains(&p.id.as_str()))
            .map(|p| p.to_saved_layout())
            .collect()
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
        let events = bar.drain_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            TopBarEvent::LayoutSelected(LayoutMode::Grid { rows: 2, cols: 3 })
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
        assert!(
            bar.tabs()
                .iter()
                .find(|t| t.profile_id == "single")
                .unwrap()
                .is_active
        );
        assert!(
            !bar.tabs()
                .iter()
                .find(|t| t.profile_id == "2x2")
                .unwrap()
                .is_active
        );
    }

    #[test]
    fn default_is_same_as_new() {
        let default = TopBar::default();
        let new = TopBar::new();
        assert_eq!(default.active_layout(), new.active_layout());
        assert_eq!(default.is_broadcast_enabled(), new.is_broadcast_enabled());
    }

    #[test]
    fn saved_profile_appears_as_tab() {
        let mut bar = TopBar::new();
        let profile =
            SavedLayoutProfile::new("custom-3x3", "3x3", LayoutMode::Grid { rows: 3, cols: 3 });
        bar.profile_manager.add_profile(profile);
        bar.refresh_tabs();
        assert_eq!(bar.tabs().len(), 4);
        assert_eq!(bar.tabs()[3].label, "3x3");
        assert!(bar.tabs()[3].is_user_saved);
    }

    #[test]
    fn remove_user_saved_tab() {
        let mut bar = TopBar::new();
        bar.profile_manager.add_profile(SavedLayoutProfile::new(
            "custom-3x3",
            "3x3",
            LayoutMode::Grid { rows: 3, cols: 3 },
        ));
        bar.refresh_tabs();
        assert_eq!(bar.tabs().len(), 4);
        bar.remove_tab(3); // Remove the user-saved tab
        assert_eq!(bar.tabs().len(), 3);
    }

    #[test]
    fn cannot_remove_default_tab() {
        let mut bar = TopBar::new();
        bar.remove_tab(0); // Try to remove "2x2" (default)
        assert_eq!(bar.tabs().len(), 3); // Unchanged
    }
}
